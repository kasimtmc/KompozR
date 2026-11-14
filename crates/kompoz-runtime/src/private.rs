//! What `#[kompozable]` and `#[named]` generate code against. Not for use by hand.

use std::marker::PhantomData;

pub use crate::remember::{group, restartable};

/// An argument that was given.
pub struct Set<T>(pub T);
/// A parameter with a default that wasn't given.
pub struct Unset;
/// A generic parameter of a function not known yet (until an argument gives it).
pub struct Infer;

#[diagnostic::on_unimplemented(
    message = "a required argument is missing: `{Self}`",
    label = "this call needs it",
    note = "`__kz_F_Missing_x` names the parameter `x` of `F`"
)]
pub trait Required {
    type Value;
    fn take(self) -> Self::Value;
}
impl<T> Required for Set<T> {
    type Value = T;
    fn take(self) -> T {
        self.0
    }
}

pub trait ArgOr<T> {
    fn or(self, default: impl FnOnce() -> T) -> T;
}
impl<T> ArgOr<T> for Set<T> {
    fn or(self, _: impl FnOnce() -> T) -> T {
        self.0
    }
}
impl<T> ArgOr<T> for Unset {
    fn or(self, default: impl FnOnce() -> T) -> T {
        default()
    }
}

/// An `impl Fn…` parameter: the closure given, or the default as a function pointer `D`.
pub trait ImplArg<D> {
    type Out;
    fn pick(self, default: D) -> Self::Out;
}
impl<D, V> ImplArg<D> for Set<V> {
    type Out = V;
    fn pick(self, _: D) -> V {
        self.0
    }
}
impl<D> ImplArg<D> for Unset {
    type Out = D;
    fn pick(self, default: D) -> D {
        default
    }
}

/// The marker type of a PascalCase function with named parameters.
pub trait Kompozable: Copy {
    type Builder;
    fn __kz_new(self) -> Self::Builder;
}

/// `(&&Probe(f)).__kz_builder()`: the builder of a marker, or a positional call of anything else callable.
pub struct Probe<T>(pub T);

pub trait ViaBuilder {
    type Builder;
    fn __kz_builder(&self) -> Self::Builder;
}
impl<M: Kompozable> ViaBuilder for &Probe<M> {
    type Builder = M::Builder;
    fn __kz_builder(&self) -> M::Builder {
        self.0.__kz_new()
    }
}

pub trait ViaFn {
    type F;
    fn __kz_builder(&self) -> Positional<Self::F, ()>;
}
impl<F: Copy> ViaFn for Probe<F> {
    type F = F;
    fn __kz_builder(&self) -> Positional<F, ()> {
        Positional(self.0, ())
    }
}

/// A call of a plain function, tuple struct or enum variant: the arguments in order.
pub struct Positional<F, A>(F, A);

pub trait Push<V> {
    type Out;
    fn push(self, v: V) -> Self::Out;
}

pub trait CallWith<A> {
    type Output;
    fn call_with(self, a: A) -> Self::Output;
}

macro_rules! tuples {
    ($($a:ident)*) => {
        impl<V, $($a),*> Push<V> for ($($a,)*) {
            type Out = ($($a,)* V,);
            #[allow(non_snake_case)]
            fn push(self, v: V) -> Self::Out {
                let ($($a,)*) = self;
                ($($a,)* v,)
            }
        }
        impl<F: FnOnce($($a),*) -> R, R, $($a),*> CallWith<($($a,)*)> for F {
            type Output = R;
            #[allow(non_snake_case)]
            fn call_with(self, a: ($($a,)*)) -> R {
                let ($($a,)*) = a;
                self($($a),*)
            }
        }
    };
}
tuples!();
tuples!(A1);
tuples!(A1 A2);
tuples!(A1 A2 A3);
tuples!(A1 A2 A3 A4);
tuples!(A1 A2 A3 A4 A5);
tuples!(A1 A2 A3 A4 A5 A6);
tuples!(A1 A2 A3 A4 A5 A6 A7);
tuples!(A1 A2 A3 A4 A5 A6 A7 A8);
tuples!(A1 A2 A3 A4 A5 A6 A7 A8 A9);
tuples!(A1 A2 A3 A4 A5 A6 A7 A8 A9 A10);
tuples!(A1 A2 A3 A4 A5 A6 A7 A8 A9 A10 A11);

macro_rules! positional_setters {
    ($($m:ident)*) => {
        impl<F, A> Positional<F, A> {
            $(
                pub fn $m<V>(self, v: V) -> Positional<F, A::Out>
                where
                    A: Push<V>,
                {
                    Positional(self.0, self.1.push(v))
                }
            )*
            #[track_caller]
            pub fn __call(self) -> F::Output
            where
                F: CallWith<A>,
            {
                self.0.call_with(self.1)
            }
        }
    };
}
positional_setters!(__arg0 __arg1 __arg2 __arg3 __arg4 __arg5 __arg6 __arg7 __arg8 __arg9 __arg10 __arg11 __last);

/// `(&&EqProbe::<T>::new()).__kz_eq()`: `T`'s `==` if it has one (what makes a composable skippable).
pub struct EqProbe<T>(PhantomData<T>);

impl<T> EqProbe<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        EqProbe(PhantomData)
    }
}

pub trait EqViaPartialEq<T> {
    fn __kz_eq(&self) -> Option<fn(&T, &T) -> bool>;
}
impl<T: PartialEq> EqViaPartialEq<T> for &EqProbe<T> {
    fn __kz_eq(&self) -> Option<fn(&T, &T) -> bool> {
        Some(<T as PartialEq>::eq)
    }
}

pub trait EqFallback<T> {
    fn __kz_eq(&self) -> Option<fn(&T, &T) -> bool>;
}
impl<T> EqFallback<T> for EqProbe<T> {
    fn __kz_eq(&self) -> Option<fn(&T, &T) -> bool> {
        None
    }
}

/// Whether `a` equals `b` by `eq`; never without one.
pub fn eq_by<T>(eq: Option<fn(&T, &T) -> bool>, a: &T, b: &T) -> bool {
    eq.is_some_and(|eq| eq(a, b))
}

/// What a closure a composable keeps captures: cloned into it, so the closure can run again and the value stay
/// where it was (Kotlin's lambdas share what they capture).
#[diagnostic::on_unimplemented(
    message = "`{Self}` is captured by a closure a composable keeps, which takes a clone of it: it must be `Clone`",
    label = "captured here",
    note = "share it (an `Rc`, a state), or write `move` on a closure at the top of the body to move it in"
)]
pub trait Capture: Clone {}
impl<T: Clone> Capture for T {}

pub fn capture<T: Capture>(value: &T) -> T {
    value.clone()
}
