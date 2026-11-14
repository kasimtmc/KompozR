//! Composition locals: values handed down the composition (Compose's CompositionLocal).

use crate::composer::{Composer, Provided};
use crate::kompozable;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A value handed down the composition: what the nearest provider above gives, else its default (made from
/// `default` when first needed, once per thread). Known by its address: define it once, as a `static`.
///
/// ```
/// # use kompoz_runtime::*;
/// static LocalAccent: KompozitionLocal<&str> = KompozitionLocal::new(|| "blue");
/// #[kompozable]
/// fn Accented() {
///     KompozitionLocalProvider(&[LocalAccent.provides("green")], || {
///         let accent = LocalAccent.current();
///     });
/// }
/// ```
pub struct KompozitionLocal<T> {
    default: fn() -> T,
}

thread_local! {
    /// The defaults made so far, by local.
    static DEFAULTS: RefCell<HashMap<usize, Rc<dyn Any>>> = RefCell::new(HashMap::new());
}

impl<T: Clone + 'static> KompozitionLocal<T> {
    /// A local whose default is `default()`, made when first read without a provider (it may panic for a local
    /// that must be provided).
    pub const fn new(default: fn() -> T) -> Self {
        KompozitionLocal { default }
    }

    fn id(&self) -> usize {
        self as *const Self as usize
    }

    /// The value: the nearest provider's, else the default. Reading it subscribes the running scope: it
    /// recomposes when a provider above it starts providing a different value.
    pub fn current(&self) -> T {
        if let Some(c) = Composer::current()
            && let Some(v) = c.read_local(self.id())
        {
            return v.downcast_ref::<T>().expect("a local's value of another type").clone();
        }
        self.default_value()
    }

    /// Its default (what is read where nothing provides it), without reading anything.
    pub fn default_value(&self) -> T {
        let id = self.id();
        if let Some(v) = DEFAULTS.with(|d| d.borrow().get(&id).cloned()) {
            return v.downcast_ref::<T>().unwrap().clone();
        }
        let v = (self.default)();
        DEFAULTS.with(|d| d.borrow_mut().insert(id, Rc::new(v.clone())));
        v
    }

    /// `local provides value`: for [`KompozitionLocalProvider`]. Readers recompose when the value given changes
    /// (compared with `==`).
    pub fn provides(&'static self, value: T) -> ProvidedValue
    where
        T: PartialEq,
    {
        ProvidedValue(Provided {
            local: self.id(),
            value: Rc::new(value),
            equals: |a, b| a.downcast_ref::<T>() == b.downcast_ref::<T>(),
            if_missing: false,
            provider: None,
        })
    }

    /// The same for a value that can't be compared: its readers recompose whenever it is provided again.
    pub fn provides_unequal(&'static self, value: T) -> ProvidedValue {
        ProvidedValue(Provided {
            local: self.id(),
            value: Rc::new(value),
            equals: |_, _| false,
            if_missing: false,
            provider: None,
        })
    }

    /// Only where no provider above gives one (Compose's providesDefault): a library's own default the app may
    /// have set already.
    pub fn provides_default(&'static self, value: T) -> ProvidedValue
    where
        T: PartialEq,
    {
        let mut p = self.provides(value);
        p.0.if_missing = true;
        p
    }
}

/// Compose's `compositionLocalOf { default }`.
pub const fn kompozition_local_of<T: Clone + 'static>(default: fn() -> T) -> KompozitionLocal<T> {
    KompozitionLocal::new(default)
}

/// Compose's `staticCompositionLocalOf { default }`: here the same as [`kompozition_local_of`] (only the scopes
/// that read it recompose when a provider gives another value).
pub const fn static_kompozition_local_of<T: Clone + 'static>(default: fn() -> T) -> KompozitionLocal<T> {
    KompozitionLocal::new(default)
}

/// A value given to a local ([`KompozitionLocal::provides`]).
#[derive(Clone)]
pub struct ProvidedValue(pub(crate) Provided);

/// Composes `content` with `values` provided to it and to what it calls.
#[kompozable(inline)]
pub fn KompozitionLocalProvider<R>(values: &[ProvidedValue], content: impl FnOnce() -> R) -> R {
    provide_here(values.iter().map(|v| v.0.clone()).collect(), content)
}

/// Composes `content` with `values` provided, in the current group.
pub(crate) fn provide_here<R>(values: Vec<Provided>, content: impl FnOnce() -> R) -> R {
    let c = Composer::require();
    let mut out = None;
    c.provide(values, || out = Some(content()));
    out.unwrap()
}

/// Every local provided where it is read (Compose's CompositionLocalContext), to provide them all again
/// somewhere else: content composed elsewhere (another window, a host of its own) that should see the same
/// theme and values.
#[derive(Clone, Default)]
pub struct KompozitionLocalContext {
    values: Vec<Provided>,
}

/// What is provided here; reading it subscribes to them all.
pub fn current_kompozition_local_context() -> KompozitionLocalContext {
    KompozitionLocalContext { values: Composer::require().locals() }
}

/// What is provided here, subscribing nothing (Compose's `currentCompositionLocalMap` as a layout node keeps it):
/// read later, outside the composition, as it was when taken.
pub fn current_kompozition_local_context_unobserved() -> KompozitionLocalContext {
    KompozitionLocalContext { values: Composer::require().locals_unobserved() }
}

impl KompozitionLocalContext {
    /// `local`'s value in it: what is provided for it, else its default. Reading it subscribes nothing.
    pub fn get<T: Clone + 'static>(&self, local: &KompozitionLocal<T>) -> T {
        match self.values.iter().rev().find(|p| p.local == local.id()) {
            Some(p) => p.value.downcast_ref::<T>().expect("a local's value of another type").clone(),
            None => local.default_value(),
        }
    }

    /// Whether something is provided for `local` in it.
    pub fn provides<T: Clone + 'static>(&self, local: &KompozitionLocal<T>) -> bool {
        self.values.iter().any(|p| p.local == local.id())
    }
}

/// Composes `content` with everything `context` holds provided.
#[kompozable(inline)]
pub fn KompozitionLocalProviderContext<R>(context: &KompozitionLocalContext, content: impl FnOnce() -> R) -> R {
    let values = context.values.iter().cloned().map(|mut p| {
        p.provider = None;
        p
    });
    provide_here(values.collect(), content)
}
