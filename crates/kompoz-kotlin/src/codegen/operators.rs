//! Kotlin's operator functions of the user's types (stage 14 WPB): `operator fun plus(o: V)` (a member, or an
//! extension `operator fun V.times(k: Int)`) is a method, and an operator on a value of that type is its call, as
//! Kotlin's conventions say:
//!
//! | Kotlin | Rust |
//! |---|---|
//! | `a + b`, `a - b`, `a * b`, `a / b`, `a % b`, `a..b`, `a..<b` | `a.plus(b)`, `a.minus(b)`, `a.times(b)`, `a.div(b)`, `a.rem(b)`, `a.range_to(b)`, `a.range_until(b)` |
//! | `-a`, `+a`, `!a` | `a.unary_minus()`, `a.unary_plus()`, `a.not()` |
//! | `a < b` (`>`, `<=`, `>=`) | `a.compare_to(b) < 0` |
//! | `x in a`, `x !in a` | `a.contains(x)`, `!a.contains(x)` |
//! | `a[i]`, `a[i] = v` | `a.get(i)`, `a.set(i, v)` |
//! | `a(x)` | `a.invoke(x)` |
//! | `a += b` | `a.plus_assign(b)` where the type has it, else `a = a.plus(b)` (a `var`) |
//! | `a++`, `--a` | `a = a.inc()`, `a = a.dec()` (as a value: the old one, the new one) |
//!
//! `infix fun` (a member or an extension) is called as written, `a shl b` → `a.shl(b)`.

use super::Gen;
use super::expr::Out;
use super::resolve::*;
use crate::ast::*;
use crate::pos::Loc;
use proc_macro2::TokenStream;
use quote::quote_spanned;

/// The operator function a binary operator is (Kotlin's conventions).
pub(crate) fn binary_operator(op: BinaryOp) -> Option<&'static str> {
    Some(match op {
        BinaryOp::Add => "plus",
        BinaryOp::Sub => "minus",
        BinaryOp::Mul => "times",
        BinaryOp::Div => "div",
        BinaryOp::Rem => "rem",
        BinaryOp::RangeTo => "rangeTo",
        BinaryOp::RangeUntil => "rangeUntil",
        BinaryOp::Lt | BinaryOp::Gt | BinaryOp::Le | BinaryOp::Ge => "compareTo",
        BinaryOp::In | BinaryOp::NotIn => "contains",
        _ => return None,
    })
}

/// The operator function an assignment's operator is, and the one it falls back to (`plusAssign`, `plus`).
fn assign_operators(op: AssignOp) -> Option<(&'static str, &'static str)> {
    Some(match op {
        AssignOp::Add => ("plusAssign", "plus"),
        AssignOp::Sub => ("minusAssign", "minus"),
        AssignOp::Mul => ("timesAssign", "times"),
        AssignOp::Div => ("divAssign", "div"),
        AssignOp::Rem => ("remAssign", "rem"),
        AssignOp::Assign => return None,
    })
}

/// The binary operator a compound assignment's is (`+=` → `+`).
pub(crate) fn compound(op: AssignOp) -> Option<BinaryOp> {
    Some(match op {
        AssignOp::Add => BinaryOp::Add,
        AssignOp::Sub => BinaryOp::Sub,
        AssignOp::Mul => BinaryOp::Mul,
        AssignOp::Div => BinaryOp::Div,
        AssignOp::Rem => BinaryOp::Rem,
        AssignOp::Assign => return None,
    })
}

fn name(n: &str, loc: Loc) -> Ident {
    Ident { name: n.to_string(), backticked: false, loc }
}

/// `receiver.name(args)` as Kotlin's tree.
pub(crate) fn method_call(receiver: &Expr, n: &str, args: &[&Expr], loc: Loc) -> Expr {
    Expr {
        kind: ExprKind::Call {
            callee: Box::new(Expr {
                kind: ExprKind::Member { receiver: Box::new(receiver.clone()), safe: false, name: name(n, loc) },
                loc,
            }),
            type_args: Vec::new(),
            args: args.iter().map(|a| Arg { name: None, spread: false, value: (*a).clone(), loc: a.loc }).collect(),
            trailing_lambda: None,
        },
        loc,
    }
}

impl Gen<'_, '_> {
    /// Whether a class's members override `equals(other: Any?)` and `hashCode()`.
    pub(crate) fn overrides_equality(&self, members: &[Member]) -> (bool, bool) {
        let has = |name: &str, params: usize| {
            members.iter().any(
                |m| matches!(m, Member::Decl(Decl::Function(f)) if f.name.name == name && f.params.len() == params),
            )
        };
        (has("equals", 1), has("hashCode", 0))
    }

    /// The bodies of a class's `PartialEq::eq` and `Hash::hash`: its `equals`/`hashCode` overrides' (Kotlin's `==` of
    /// it calls `equals`, its hash is `hashCode()`'s), else `eq`/`hash` as given.
    pub(crate) fn equality_bodies(
        &self,
        members: &[Member],
        eq: TokenStream,
        hash: TokenStream,
        span: proc_macro2::Span,
    ) -> (TokenStream, TokenStream) {
        let (equals, hash_code) = self.overrides_equality(members);
        let eq = if equals {
            let kt = self.kt(span);
            quote_spanned!(span=> self.equals_kt(::core::option::Option::Some(
                ::std::rc::Rc::new(::core::clone::Clone::clone(other)) as ::std::rc::Rc<dyn #kt::KtObject>
            )))
        } else {
            eq
        };
        let hash = if hash_code { quote_spanned!(span=> state.write_i32(self.hash_code_kt())) } else { hash };
        (eq, hash)
    }

    /// Whether a value of type `ty` has the user's operator function `kotlin` (its class's member, or an extension
    /// of the file for its type).
    pub(crate) fn has_operator(&self, ty: &Ty, kotlin: &str) -> bool {
        self.operator_ret(ty, kotlin).is_some()
    }

    /// What the user's operator function `kotlin` of a value of type `ty` gives.
    pub(crate) fn operator_ret(&self, ty: &Ty, kotlin: &str) -> Option<Ty> {
        let owner = ty.non_null().owner()?.to_string();
        if self.user_class_rust(&owner).is_some()
            && let Some(i) = members_of(&owner, kotlin).find(|i| i.kind == crate::index::Kind::Method)
        {
            return Some(Ty::from_rust(i.ret));
        }
        self.file.extension(kotlin, Some(&owner)).map(|f| f.ret.clone())
    }

    /// `lhs op rhs` of a user's type with that operator function.
    pub(crate) fn user_binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, loc: Loc) -> Option<Out> {
        let f = binary_operator(op)?;
        let span = self.span(loc);
        let (recv, arg) = if matches!(op, BinaryOp::In | BinaryOp::NotIn) { (rhs, lhs) } else { (lhs, rhs) };
        if !self.has_operator(&self.peek_ty(recv), f) {
            return None;
        }
        let call = self.operator_call(recv, f, &[arg], loc);
        let ts = call.ts;
        Some(match op {
            BinaryOp::Lt => Out::plain(quote_spanned!(span=> (#ts < 0)), Ty::Bool),
            BinaryOp::Gt => Out::plain(quote_spanned!(span=> (#ts > 0)), Ty::Bool),
            BinaryOp::Le => Out::plain(quote_spanned!(span=> (#ts <= 0)), Ty::Bool),
            BinaryOp::Ge => Out::plain(quote_spanned!(span=> (#ts >= 0)), Ty::Bool),
            BinaryOp::NotIn => Out::plain(quote_spanned!(span=> !(#ts)), Ty::Bool),
            BinaryOp::In => Out::plain(ts, Ty::Bool),
            _ => Out { ts, ..call },
        })
    }

    /// `-a`, `+a`, `!a` of a user's type with that operator function.
    pub(crate) fn user_prefix(&mut self, op: PrefixOp, operand: &Expr, loc: Loc) -> Option<Out> {
        let f = match op {
            PrefixOp::Minus => "unaryMinus",
            PrefixOp::Plus => "unaryPlus",
            PrefixOp::Not => "not",
            _ => return None,
        };
        if !self.has_operator(&self.peek_ty(operand), f) {
            return None;
        }
        Some(self.operator_call(operand, f, &[], loc))
    }

    /// `a[i]` of a user's type with `operator fun get`.
    pub(crate) fn user_index(&mut self, receiver: &Expr, indices: &[Expr], loc: Loc) -> Option<Out> {
        if !self.has_operator(&self.peek_ty(receiver), "get") {
            return None;
        }
        let values: Vec<&Expr> = indices.iter().collect();
        Some(self.operator_call(receiver, "get", &values, loc))
    }

    /// `a(args)` of a local of a user's type with `operator fun invoke`.
    pub(crate) fn user_invoke(&mut self, callee: &Expr, args: &[Arg], loc: Loc) -> Option<Out> {
        if !self.has_operator(&self.peek_ty(callee), "invoke") {
            return None;
        }
        let values: Vec<&Expr> = args.iter().map(|a| &a.value).collect();
        let call = method_call(callee, "invoke", &values, loc);
        Some(self.expr(&call))
    }

    /// `target op= value` of a user's type with that operator function, `a[i] = v` with `set`.
    pub(crate) fn user_assign(&mut self, target: &Expr, op: AssignOp, value: &Expr, loc: Loc) -> Option<TokenStream> {
        let span = self.span(loc);
        if op == AssignOp::Assign {
            let ExprKind::Index { receiver, indices } = &target.kind else { return None };
            if !self.has_operator(&self.peek_ty(receiver), "set") {
                return None;
            }
            let mut values: Vec<&Expr> = indices.iter().collect();
            values.push(value);
            let call = self.operator_call(receiver, "set", &values, loc).ts;
            return Some(quote_spanned!(span=> #call;));
        }
        let (assign, plain) = assign_operators(op)?;
        let ty = self.peek_ty(target);
        if self.has_operator(&ty, assign) {
            let call = self.operator_call(target, assign, &[value], loc).ts;
            return Some(quote_spanned!(span=> #call;));
        }
        if self.has_operator(&ty, plain) {
            let made = method_call(target, plain, &[value], loc);
            return Some(self.assign(target, AssignOp::Assign, &made, loc));
        }
        None
    }

    /// `a++`/`a--` (`++a`, `--a`) as a statement, of a user's type with `operator fun inc`/`dec`.
    pub(crate) fn user_step_stmt(&mut self, operand: &Expr, inc: bool, loc: Loc) -> Option<TokenStream> {
        let f = if inc { "inc" } else { "dec" };
        if !self.has_operator(&self.peek_ty(operand), f) {
            return None;
        }
        let made = method_call(operand, f, &[], loc);
        let set = self.assign(operand, AssignOp::Assign, &made, loc);
        let span = self.span(loc);
        Some(quote_spanned!(span=> { #set }))
    }

    /// `x++` (`pre`: `++x`; `inc` else `--`) as a value of a property (a member, an implicit receiver's): its value
    /// read, the next one set (`KtInc`), the old one (or the new one) given.
    pub(crate) fn step_value(&mut self, operand: &Expr, inc: bool, pre: bool, loc: Loc) -> Out {
        let span = self.span(loc);
        let kt = self.kt(span);
        let ty = self.peek_ty(operand);
        let v = self.value(operand);
        self.push_scope();
        self.declare("__kz_step", "__kz_step".into(), LocalKind::Val, ty.clone());
        let step = Expr { kind: ExprKind::Name(name("__kz_step", loc)), loc };
        let set = self.assign(operand, AssignOp::Assign, &step, loc);
        self.pop_scope();
        let f = if inc { quote_spanned!(span=> inc) } else { quote_spanned!(span=> dec) };
        let ts = if pre {
            quote_spanned!(span=> { let __kz_step = #kt::KtInc::#f(#v); #set ::core::clone::Clone::clone(&__kz_step) })
        } else {
            quote_spanned!(span=> {
                let __kz_old = #v;
                let __kz_step = #kt::KtInc::#f(::core::clone::Clone::clone(&__kz_old));
                #set
                __kz_old
            })
        };
        Out::plain(ts, ty)
    }

    /// `a++` (`pre`: `++a`) as a value, of a local of a user's type with `operator fun inc`/`dec`: the old value (the
    /// new one), the local given the new one.
    pub(crate) fn user_step_expr(&mut self, operand: &Expr, inc: bool, pre: bool, loc: Loc) -> Option<Out> {
        let f = if inc { "inc" } else { "dec" };
        let ty = self.peek_ty(operand);
        if !self.has_operator(&ty, f) {
            return None;
        }
        let span = self.span(loc);
        let old = self.value(operand);
        let made = method_call(operand, f, &[], loc);
        let set = self.assign(operand, AssignOp::Assign, &made, loc);
        let v = self.fresh("v", span);
        let ts = if pre {
            let new = self.value(operand);
            quote_spanned!(span=> { #set #new })
        } else {
            quote_spanned!(span=> { let #v = #old; #set #v })
        };
        Some(Out::plain(ts, ty))
    }
}
