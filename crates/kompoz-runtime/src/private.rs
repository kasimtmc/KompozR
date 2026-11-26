//! What `#[kompozable]` and `#[named]` generate code against. Not for use by hand.

use std::marker::PhantomData;
use std::panic::Location;
use std::rc::Rc;
use std::sync::Arc;

pub use crate::composer::{Marker, current_marker, end_to_marker};
pub use crate::remember::{Memoized, group, restartable};

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

/// The marker type of a PascalCase composable (its name a constant of it), which hands out its builder.
pub trait Kompozable: Copy {
    type Builder;
    fn __kz_new(self) -> Self::Builder;
}

/// `(&&Probe(f)).__kz_builder()`: the builder of a composable's marker, or a positional call of anything else
/// callable (a function, `#[named]` ones taking the parameters without a default; a tuple struct; a variant).
pub struct Probe<T>(pub T);

pub trait ViaBuilder {
    type Builder;
    fn __kz_builder(&self) -> Self::Builder;
    /// The builder for a call with named arguments (opened by [`named`]).
    fn __kz_named(&self) -> Named<Self::Builder> {
        Named(self.__kz_builder())
    }
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
    /// A call with named arguments of what isn't a composable: an error from [`named`], as nothing is known of its
    /// parameters.
    fn __kz_named(&self) -> Self::F;
}
impl<F: Copy> ViaFn for Probe<F> {
    type F = F;
    fn __kz_builder(&self) -> Positional<F, ()> {
        Positional(self.0, ())
    }
    fn __kz_named(&self) -> F {
        self.0
    }
}

/// A composable's builder, for a call with named arguments.
pub struct Named<B>(B);

/// What a call with named arguments opens: a composable's builder. Anything else called with named arguments (what
/// [`ViaFn`] hands on) isn't one, and [`named`] reports it.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't known to take named arguments",
    label = "called with named arguments here",
    note = "a #[named] PascalCase function's named calls go through its type (`Name::__kz_builder()`): the macros know it from the API index (`cargo xtask kotlin-index`) or when it is defined in the same body",
    note = "outside those, call its builder yourself: `Name::__kz_builder().param(value).__call()`"
)]
pub trait NamedCall {
    type Builder;
    fn __kz_open(self) -> Self::Builder;
}
impl<B> NamedCall for Named<B> {
    type Builder = B;
    fn __kz_open(self) -> B {
        self.0
    }
}

/// `named((&&Probe(f)).__kz_named())`: the builder of a call with named arguments. (A free function's bound, so what
/// rustc says when it isn't met reads the same in every rustc.)
pub fn named<C: NamedCall>(call: C) -> C::Builder {
    call.__kz_open()
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
    ($($m:ident $memo:ident $($some:ident)?),*) => {
        #[allow(non_snake_case)]
        impl<F, A> Positional<F, A> {
            $(
                pub fn $m<V>(self, v: V) -> Positional<F, A::Out>
                where
                    A: Push<V>,
                {
                    Positional(self.0, self.1.push(v))
                }
                /// A memoized closure: nothing here keeps its identity.
                pub fn $memo<K, V>(self, keys: K, v: V) -> Positional<F, A::Out>
                where
                    A: Push<V>,
                {
                    let _ = keys;
                    Positional(self.0, self.1.push(v))
                }
                $(
                    /// An argument written `Some(…)`: as any other here.
                    pub fn $some<V>(self, v: V) -> Positional<F, A::Out>
                    where
                        A: Push<V>,
                    {
                        Positional(self.0, self.1.push(v))
                    }
                )?
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
positional_setters!(
    __arg0 __kz_memo___arg0 __kz_some___arg0, __arg1 __kz_memo___arg1 __kz_some___arg1,
    __arg2 __kz_memo___arg2 __kz_some___arg2, __arg3 __kz_memo___arg3 __kz_some___arg3,
    __arg4 __kz_memo___arg4 __kz_some___arg4, __arg5 __kz_memo___arg5 __kz_some___arg5,
    __arg6 __kz_memo___arg6 __kz_some___arg6, __arg7 __kz_memo___arg7 __kz_some___arg7,
    __arg8 __kz_memo___arg8 __kz_some___arg8, __arg9 __kz_memo___arg9 __kz_some___arg9,
    __arg10 __kz_memo___arg10 __kz_some___arg10, __arg11 __kz_memo___arg11 __kz_some___arg11,
    // (what goes to `__last` is a closure: no `Some(…)` twin)
    __last __kz_memo___last
);

/// `(&&&EqProbe::<T>::new()).__kz_eq()`: how a composable's arguments of type `T` compare (what makes it
/// skippable), as Compose's strong skipping compares them: by `==` if `T` has it, else by instance if `T` is a
/// shared handle ([`Instance`]: an `Rc`, an `Arc`), else never equal.
pub struct EqProbe<T>(PhantomData<T>);

impl<T> EqProbe<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        EqProbe(PhantomData)
    }
    /// The probe of `value`'s type (where a macro has a value, not its type).
    pub fn of(_value: &T) -> Self {
        EqProbe(PhantomData)
    }
}

pub trait EqViaPartialEq<T> {
    fn __kz_eq(&self) -> Option<fn(&T, &T) -> bool>;
}
impl<T: PartialEq> EqViaPartialEq<T> for &&EqProbe<T> {
    fn __kz_eq(&self) -> Option<fn(&T, &T) -> bool> {
        Some(<T as PartialEq>::eq)
    }
}

pub trait EqViaInstance<T> {
    fn __kz_eq(&self) -> Option<fn(&T, &T) -> bool>;
}
impl<T: Instance> EqViaInstance<T> for &EqProbe<T> {
    fn __kz_eq(&self) -> Option<fn(&T, &T) -> bool> {
        Some(<T as Instance>::same_instance)
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

/// A shared handle, compared by what it points at when what it holds has no `==` (Compose's instance equality,
/// `===`, of an unstable value).
pub trait Instance {
    fn same_instance(&self, other: &Self) -> bool;
}
impl<T: ?Sized> Instance for Rc<T> {
    fn same_instance(&self, other: &Self) -> bool {
        Rc::ptr_eq(self, other)
    }
}
impl<T: ?Sized> Instance for std::rc::Weak<T> {
    fn same_instance(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}
impl<T: ?Sized> Instance for Arc<T> {
    fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(self, other)
    }
}
impl<T: ?Sized> Instance for std::sync::Weak<T> {
    fn same_instance(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

pub use crate::remember::remember_updated_state_with;
pub use crate::state::{StateProbe, StateViaConfined, StateViaShared, policy_of};

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

// ---- Memoized closures (Compose's strong skipping) ----

/// What a closure's identity is kept by: one key per value it captures, in a list (`(k1, (k2, ()))`). Equal keys,
/// an equal closure (the same code over equal values).
pub trait Keys {
    fn same(&self, other: &Self) -> bool;
}
impl Keys for () {
    fn same(&self, _: &()) -> bool {
        true
    }
}
impl<A: Key, B: Keys> Keys for (A, B) {
    fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0) && self.1.same(&other.1)
    }
}

/// A captured value as a closure's identity compares it.
pub trait Key {
    fn same(&self, other: &Self) -> bool;
}

/// A value with `==`, kept as it was captured.
pub struct ByValue<T>(pub T);
impl<T: PartialEq> Key for ByValue<T> {
    fn same(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

/// A shared handle without `==`: the same one or not.
pub struct ByInstance<T>(pub T);
impl<T: Instance> Key for ByInstance<T> {
    fn same(&self, other: &Self) -> bool {
        self.0.same_instance(&other.0)
    }
}

/// What can't be compared (a value with neither, one the macros can't see): never the same, so the closure is a
/// new one each time.
pub struct Unequal;
impl Key for Unequal {
    fn same(&self, _: &Self) -> bool {
        false
    }
}

/// A closure parameter passed on, or captured: its own identity (none: never the same).
pub struct IdKey(pub Option<u64>);
impl Key for IdKey {
    fn same(&self, other: &Self) -> bool {
        self.0.is_some() && self.0 == other.0
    }
}

/// `(&&&KeyProbe(&value)).__kz_key()`: the key of a captured value (by value, by instance, or [`Unequal`]).
pub struct KeyProbe<'a, T>(pub &'a T);

pub trait KeyViaPartialEq<T> {
    fn __kz_key(&self) -> ByValue<T>;
}
impl<T: PartialEq + Clone> KeyViaPartialEq<T> for &&KeyProbe<'_, T> {
    fn __kz_key(&self) -> ByValue<T> {
        ByValue(self.0.clone())
    }
}

pub trait KeyViaInstance<T> {
    fn __kz_key(&self) -> ByInstance<T>;
}
impl<T: Instance + Clone> KeyViaInstance<T> for &KeyProbe<'_, T> {
    fn __kz_key(&self) -> ByInstance<T> {
        ByInstance(self.0.clone())
    }
}

pub trait KeyFallback {
    fn __kz_key(&self) -> Unequal;
}
impl<T> KeyFallback for KeyProbe<'_, T> {
    fn __kz_key(&self) -> Unequal {
        Unequal
    }
}

/// The identity of a closure passed where it is kept (a restartable composable's closure parameter, a
/// `#[memoized]` one): the same as at the last composition of this place while `keys` are the same, a new one
/// when they change (or can't be compared); none outside a composition.
#[track_caller]
pub fn memo_id<K: Keys + 'static>(keys: K) -> Option<u64> {
    crate::remember::memo_id_at(Location::caller(), keys)
}

/// The identity of a closure parameter's default (its `#[default(…)]` closure, given when no argument is): always
/// the same one, as Kotlin's default lambda (`topBar: @Composable () -> Unit = {}`) is one instance — a default
/// can't capture the other arguments, so each call's default does the same. (Memoized identities count up from 1:
/// never this one.)
pub const DEFAULT_CLOSURE_ID: Option<u64> = Some(u64::MAX);

/// Whether two identities are the same one (none never is).
pub fn same_id(a: Option<u64>, b: Option<u64>) -> bool {
    a.is_some() && a == b
}

/// A `#[memoized]` parameter as its body sees it.
pub fn memoized<T>(value: T, id: Option<u64>) -> Memoized<T> {
    Memoized::new(value, id)
}
