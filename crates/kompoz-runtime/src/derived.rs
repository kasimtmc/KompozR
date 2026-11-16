//! `derived_state_of`: a value computed from states, whose readers recompose only when it changes.

use crate::composer::{Composer, ScopeId};
use crate::read::record_read;
use crate::snapshot::StateKey;
use crate::state::{SnapshotMutationPolicy, State, StateSource, structural_equality_policy};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// A value computed from other states. Readers recompose only when the computed value itself changes, not on
/// every change of its inputs. Made in a composition (in `remember`), it observes its inputs and recomputes as
/// they change; elsewhere it computes at every read.
///
/// ```
/// # use kompoz_runtime::*;
/// # fn content(index: MutableState<usize>) {
/// let show_button = remember(|| derived_state_of(move || index.get() > 0));
/// if show_button.get() { /* … */ }
/// # }
/// ```
pub struct DerivedState<T> {
    inner: Rc<Inner<T>>,
}

impl<T> Clone for DerivedState<T> {
    fn clone(&self) -> Self {
        DerivedState { inner: self.inner.clone() }
    }
}

struct Inner<T> {
    key: StateKey,
    calc: Box<dyn Fn() -> T>,
    equivalent: Option<fn(&T, &T) -> bool>,
    value: RefCell<Option<T>>,
    valid: Cell<bool>,
    /// The scope recording its reads, in the composition it was made in.
    observer: Cell<Option<ScopeId>>,
    composer: Weak<Composer>,
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        if let Some(s) = self.observer.take()
            && let Some(c) = self.composer.upgrade()
        {
            c.remove_observer(s);
        }
    }
}

impl<T: Clone + 'static> Inner<T> {
    /// Computes the value again; whether it changed.
    fn recompute(&self) -> bool {
        let next = match (self.observer.get(), self.composer.upgrade()) {
            (Some(s), Some(c)) => c.observe_into(s, || (self.calc)()),
            _ => (self.calc)(),
        };
        let changed = match &*self.value.borrow() {
            Some(old) => !self.equivalent.is_some_and(|eq| eq(old, &next)),
            None => true,
        };
        *self.value.borrow_mut() = Some(next);
        self.valid.set(true);
        changed
    }

    fn read(&self, subscribe: bool) -> std::cell::Ref<'_, T> {
        if !self.valid.get() || self.observer.get().is_none() {
            self.recompute(); // unobserved: always fresh
        }
        if subscribe {
            record_read(self.key);
        }
        std::cell::Ref::map(self.value.borrow(), |v| v.as_ref().unwrap())
    }
}

impl<T: Clone + 'static> DerivedState<T> {
    pub fn new(calc: impl Fn() -> T + 'static, policy: SnapshotMutationPolicy<T>) -> Self {
        let composer = Composer::current();
        let inner = Rc::new(Inner {
            key: StateKey::new(),
            calc: Box::new(calc),
            equivalent: policy.equivalent,
            value: RefCell::new(None),
            valid: Cell::new(false),
            observer: Cell::new(None),
            composer: composer.as_ref().map_or_else(Weak::new, Rc::downgrade),
        });
        if let Some(c) = composer {
            let weak = Rc::downgrade(&inner);
            let composer = Rc::downgrade(&c);
            let s = c.new_observer(Rc::new(move || {
                if let (Some(inner), Some(c)) = (weak.upgrade(), composer.upgrade())
                    && inner.recompute()
                {
                    c.invalidate_readers(inner.key);
                }
            }));
            inner.observer.set(Some(s));
        }
        DerivedState { inner }
    }

    pub fn get(&self) -> T {
        self.inner.read(true).clone()
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.inner.read(true))
    }

    /// Without subscribing.
    pub fn peek(&self) -> T {
        self.inner.read(false).clone()
    }

    /// Its value (Compose's `state.value`): the same as [`get`](DerivedState::get).
    pub fn value(&self) -> T {
        self.get()
    }
}

impl<T> PartialEq for DerivedState<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: Clone + 'static> StateSource<T> for DerivedState<T> {
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&T)) {
        f(&self.inner.read(subscribe));
    }
    fn identity(&self) -> *const () {
        Rc::as_ptr(&self.inner) as *const ()
    }
}

impl<T: Clone + 'static> From<DerivedState<T>> for State<T> {
    fn from(d: DerivedState<T>) -> State<T> {
        State::from_source(d)
    }
}

/// `derivedStateOf { … }`: its readers recompose when the value changes (`==`).
pub fn derived_state_of<T: PartialEq + Clone + 'static>(calc: impl Fn() -> T + 'static) -> DerivedState<T> {
    DerivedState::new(calc, structural_equality_policy())
}

/// `derivedStateOf(policy) { … }`: when a new value counts as the old one, by `policy`.
pub fn derived_state_of_with_policy<T: Clone + 'static>(
    policy: SnapshotMutationPolicy<T>,
    calc: impl Fn() -> T + 'static,
) -> DerivedState<T> {
    DerivedState::new(calc, policy)
}
