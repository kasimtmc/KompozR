//! Composition locals: values handed down the composition (Compose's CompositionLocal).

use crate::composer::{Composer, Provided, ValueHolder};
use crate::kompozable;
use crate::state::SnapshotMutationPolicy;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A value handed down the composition: what the nearest provider above gives, else its default (made from
/// `default` when first needed, once per thread). Known by its address: define it once, as a `static`.
///
/// A dynamic local (Compose's `compositionLocalOf`, [`KompozitionLocal::new`]) keeps what its provider gives in a
/// holder the provider sets another value in: reading it in a composable recomposes that composable when the value
/// changes, and reading it elsewhere (a layout node's measure, placement or drawing, through the locals the node
/// keeps) is observed as any state read is, so that phase runs again without anything composing. A static local
/// (Compose's `staticCompositionLocalOf`, [`KompozitionLocal::new_static`]) is for values that hardly change: its
/// provider giving another one composes again whoever read it, the layouts that keep it included.
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
    /// A default worked out from other locals where it is read (Compose's `compositionLocalWithComputedDefaultOf`).
    computed: Option<fn(&KompozitionLocalAccessorScope) -> T>,
    dynamic: bool,
    /// A dynamic local's mutation policy (none: `==`, or never the same for a value given unequal).
    policy: Option<SnapshotMutationPolicy<T>>,
}

thread_local! {
    /// The defaults made so far, by local.
    static DEFAULTS: RefCell<HashMap<usize, Rc<dyn Any>>> = RefCell::new(HashMap::new());
}

/// A computed local's default is never made from a plain default.
fn no_plain_default<T>() -> T {
    unreachable!("kompoz: a computed local's default is computed")
}

/// Where a computed local's default reads the other locals (Compose's CompositionLocalAccessorScope): the
/// composition reading it, the context of a node reading it, or (outside of both) their defaults.
pub struct KompozitionLocalAccessorScope<'a> {
    source: AccessorSource<'a>,
}

enum AccessorSource<'a> {
    /// The composition running: what is provided where the local is read, the read subscribing the running scope.
    Composer,
    /// A context's (a layout node's locals): read, or peeked at.
    Context(&'a KompozitionLocalContext, bool),
    /// No providers: the defaults.
    Defaults,
}

impl KompozitionLocalAccessorScope<'_> {
    /// `local`'s value where the computed local is read (Compose's `CompositionLocal.currentValue`).
    pub fn current_value<U: Clone + 'static>(&self, local: &KompozitionLocal<U>) -> U {
        match &self.source {
            AccessorSource::Composer => local.current(),
            AccessorSource::Context(context, true) => context.get(local),
            AccessorSource::Context(context, false) => context.peek(local),
            AccessorSource::Defaults => local.default_value(),
        }
    }
}

impl<T: Clone + 'static> KompozitionLocal<T> {
    /// A dynamic local (Compose's `compositionLocalOf`) whose default is `default()`, made when first read without
    /// a provider (it may panic for a local that must be provided).
    pub const fn new(default: fn() -> T) -> Self {
        KompozitionLocal { default, computed: None, dynamic: true, policy: None }
    }

    /// A dynamic local whose provider's values are the same when `policy` says so (Compose's
    /// `compositionLocalOf(policy)`): one the same as the value held sets nothing and tells no one.
    pub const fn with_policy(policy: SnapshotMutationPolicy<T>, default: fn() -> T) -> Self {
        KompozitionLocal { default, computed: None, dynamic: true, policy: Some(policy) }
    }

    /// A static local (Compose's `staticCompositionLocalOf`).
    pub const fn new_static(default: fn() -> T) -> Self {
        KompozitionLocal { default, computed: None, dynamic: false, policy: None }
    }

    /// A dynamic local whose default `compute` works out from other locals where it is read (Compose's
    /// `compositionLocalWithComputedDefaultOf`): read without a provider, it is computed again each time, reading
    /// them there (`scope.current_value(&LocalDensity)`), and those reads subscribe as reading them would.
    pub const fn new_computed(compute: fn(&KompozitionLocalAccessorScope) -> T) -> Self {
        KompozitionLocal { default: no_plain_default::<T>, computed: Some(compute), dynamic: true, policy: None }
    }

    pub(crate) fn id(&self) -> usize {
        self as *const Self as usize
    }

    /// Whether it is dynamic (Compose's compositionLocalOf), not static.
    pub fn is_dynamic(&self) -> bool {
        self.dynamic
    }

    /// The value: the nearest provider's, else the default. Reading it subscribes the running scope: it
    /// recomposes when the provider above it gives a different value.
    pub fn current(&self) -> T {
        if let Some(c) = Composer::current() {
            if let Some(v) = c.read_local(self.id()) {
                return v.downcast_ref::<T>().expect("a local's value of another type").clone();
            }
            if let Some(compute) = self.computed {
                return compute(&KompozitionLocalAccessorScope { source: AccessorSource::Composer });
            }
        }
        self.default_value()
    }

    /// Its default (what is read where nothing provides it), without reading anything: a computed one's from
    /// the other locals' defaults.
    pub fn default_value(&self) -> T {
        if let Some(compute) = self.computed {
            return compute(&KompozitionLocalAccessorScope { source: AccessorSource::Defaults });
        }
        let id = self.id();
        if let Some(v) = DEFAULTS.with(|d| d.borrow().get(&id).cloned()) {
            return v.downcast_ref::<T>().unwrap().clone();
        }
        let v = (self.default)();
        DEFAULTS.with(|d| d.borrow_mut().insert(id, Rc::new(v.clone())));
        v
    }

    /// Its default where `context` is (a computed one from the context's other locals, read or peeked at).
    fn default_in(&self, context: &KompozitionLocalContext, read: bool) -> T {
        match self.computed {
            Some(compute) => compute(&KompozitionLocalAccessorScope { source: AccessorSource::Context(context, read) }),
            None => self.default_value(),
        }
    }

    /// The comparison its provider's values go by: its policy's, else `fallback`.
    fn equals(&self, fallback: fn(&T, &T) -> bool) -> Rc<dyn Fn(&dyn Any, &dyn Any) -> bool> {
        let same = match self.policy {
            Some(policy) => policy.equivalent,
            None => Some(fallback),
        };
        Rc::new(move |a: &dyn Any, b: &dyn Any| match (same, a.downcast_ref::<T>(), b.downcast_ref::<T>()) {
            (Some(same), Some(a), Some(b)) => same(a, b),
            _ => false,
        })
    }

    fn provided(&'static self, value: T, equals: Rc<dyn Fn(&dyn Any, &dyn Any) -> bool>) -> ProvidedValue {
        ProvidedValue(Provided {
            local: self.id(),
            holder: ValueHolder::new(Rc::new(value)),
            equals,
            if_missing: false,
            dynamic: self.dynamic,
            shared: false,
        })
    }

    /// `local provides value`: for [`KompozitionLocalProvider`]. Readers are told when the value given changes
    /// (compared with `==`, or by the local's policy).
    pub fn provides(&'static self, value: T) -> ProvidedValue
    where
        T: PartialEq,
    {
        self.provided(value, self.equals(|a, b| a == b))
    }

    /// The same for a value that can't be compared: its readers are told whenever it is provided again (unless
    /// the local's policy says it is the same).
    pub fn provides_unequal(&'static self, value: T) -> ProvidedValue {
        let never: fn(&T, &T) -> bool = |_, _| false;
        self.provided(value, self.equals(never))
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

/// Compose's `compositionLocalOf { default }`: a dynamic local.
pub const fn kompozition_local_of<T: Clone + 'static>(default: fn() -> T) -> KompozitionLocal<T> {
    KompozitionLocal::new(default)
}

/// Compose's `compositionLocalOf(policy) { default }`.
pub const fn kompozition_local_of_with_policy<T: Clone + 'static>(
    policy: SnapshotMutationPolicy<T>,
    default: fn() -> T,
) -> KompozitionLocal<T> {
    KompozitionLocal::with_policy(policy, default)
}

/// Compose's `compositionLocalWithComputedDefaultOf { compute }`: a dynamic local whose default is worked out
/// from other locals where it is read.
pub const fn kompozition_local_with_computed_default_of<T: Clone + 'static>(
    compute: fn(&KompozitionLocalAccessorScope) -> T,
) -> KompozitionLocal<T> {
    KompozitionLocal::new_computed(compute)
}

/// Compose's `staticCompositionLocalOf { default }`: a static local.
pub const fn static_kompozition_local_of<T: Clone + 'static>(default: fn() -> T) -> KompozitionLocal<T> {
    KompozitionLocal::new_static(default)
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

/// Every local provided where it is taken (Compose's CompositionLocalContext and CompositionLocalMap), to read
/// them later or provide them all again somewhere else: content composed elsewhere (another window, a host of
/// its own) that should see the same theme and values. A dynamic local's value is read from it as it is now.
#[derive(Clone, Default)]
pub struct KompozitionLocalContext {
    values: Vec<Provided>,
}

/// What is provided here; reading it subscribes to them all.
pub fn current_kompozition_local_context() -> KompozitionLocalContext {
    KompozitionLocalContext { values: Composer::require().locals() }
}

/// What is provided here, subscribing nothing: read later, outside the composition (dynamic locals as they are
/// then; static ones as they were when taken).
pub fn current_kompozition_local_context_unobserved() -> KompozitionLocalContext {
    KompozitionLocalContext { values: Composer::require().locals_unobserved() }
}

/// What is provided here, as a layout node keeps it (Compose's `currentCompositionLocalMap`): dynamic locals are
/// read from it live (whoever reads one is told when it changes); the running scope is subscribed to the static
/// ones (and to what a context provided again gives), so it composes again, and its node keeps the new ones, when
/// one changes.
pub fn current_kompozition_local_map() -> KompozitionLocalContext {
    KompozitionLocalContext { values: Composer::require().locals_kept() }
}

impl KompozitionLocalContext {
    pub(crate) fn from_values(values: Vec<Provided>) -> Self {
        KompozitionLocalContext { values }
    }

    /// `local`'s value in it: what is provided for it, else its default. A dynamic local's is read as it is now,
    /// and the read is told to whoever observes reads (a composable, a layout's measure, a drawing): it is told
    /// when the provider gives another value. A static local's is read by no one.
    pub fn get<T: Clone + 'static>(&self, local: &KompozitionLocal<T>) -> T {
        match self.values.iter().rev().find(|p| p.local == local.id()) {
            Some(p) => {
                let value = if p.dynamic { p.holder.read() } else { p.holder.peek() };
                value.downcast_ref::<T>().expect("a local's value of another type").clone()
            }
            None => local.default_in(self, true),
        }
    }

    /// The same, read by no one.
    pub fn peek<T: Clone + 'static>(&self, local: &KompozitionLocal<T>) -> T {
        match self.values.iter().rev().find(|p| p.local == local.id()) {
            Some(p) => p.holder.peek().downcast_ref::<T>().expect("a local's value of another type").clone(),
            None => local.default_in(self, false),
        }
    }

    /// Whether something is provided for `local` in it.
    pub fn provides<T: Clone + 'static>(&self, local: &KompozitionLocal<T>) -> bool {
        self.values.iter().any(|p| p.local == local.id())
    }
}

impl PartialEq for KompozitionLocalContext {
    /// The same locals from the same holders (Compose's CompositionLocalMap equality: a dynamic local given
    /// another value is the same, a static one isn't).
    fn eq(&self, other: &Self) -> bool {
        self.values.len() == other.values.len()
            && self
                .values
                .iter()
                .zip(&other.values)
                .all(|(a, b)| a.local == b.local && Rc::ptr_eq(&a.holder, &b.holder))
    }
}

/// Composes `content` with everything `context` holds provided: a dynamic local's holder is the context's, so what
/// reads it here is told as its provider there gives another value.
#[kompozable(inline)]
pub fn KompozitionLocalProviderContext<R>(context: &KompozitionLocalContext, content: impl FnOnce() -> R) -> R {
    let values = context.values.iter().cloned().map(|mut p| {
        p.shared = true;
        p
    });
    provide_here(values.collect(), content)
}
