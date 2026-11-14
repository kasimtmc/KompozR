//! What the generated builders use.

/// An argument that was given.
pub struct Set<T>(pub T);
/// A parameter with a default that wasn't given.
pub struct Unset;

#[diagnostic::on_unimplemented(
    message = "a required argument is missing: `{Self}`",
    label = "this call needs it",
    note = "`Missing_x` names the parameter `x`"
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
positional_setters!(__arg0 __arg1 __arg2 __arg3 __arg4 __arg5 __arg6 __arg7 __last);
