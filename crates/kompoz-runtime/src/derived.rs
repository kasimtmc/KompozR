//! `derived_state_of`: a value computed from states, whose readers recompose only when it changes.

use crate::composer::{Composer, ScopeId};
use crate::read::{Dependency, DerivedDependency, collect_dependencies, collecting_dependencies, tell_observer};
use crate::snapshot::{StateKey, read_epoch};
use crate::state::{SnapshotMutationPolicy, State, StateSource, structural_equality_policy};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

/// A value computed from other states. Readers recompose only when the computed value itself changes, not on
/// every change of its inputs. Made in a composition (in `remember`), it keeps its value: it observes its inputs
/// (computed again as the composition takes their changes, its readers told when the value changed), and it is
/// checked as it is read, as Compose's DerivedSnapshotState: the records of the states it read, and the values
/// of the derived states it read, are compared with what the snapshot read in sees now — anything written since
/// (right after a write, before the composition took it; from a coroutine; in another snapshot) computes it
/// again there, its readers told if the value changed. Made elsewhere, it computes at every read (what it reads
/// read by its reader).
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
    /// What computing it read last (Compose's ResultRecord.dependencies).
    dependencies: RefCell<Vec<Dependency>>,
    /// Every key its last computation read.
    read_keys: RefCell<Vec<StateKey>>,
    /// Whether every key it read is among its dependencies (a local's holder isn't: its change reaches it only
    /// through the composition).
    all_tracked: Cell<bool>,
    /// Where and after how many writes it was last found up to date (Compose's validSnapshotId and
    /// validSnapshotWriteCount): read there again with no write since, it is.
    validated_at: Cell<Option<(i64, u64)>>,
    /// One more at each change of its value (what a derived state reading it compares).
    version: Cell<u64>,
    /// One more at each computation (a check finding it computed meanwhile, by a reader it told, stops).
    computations: Cell<u64>,
    /// The scope recording its reads, in the composition it was made in.
    observer: Cell<Option<ScopeId>>,
    composer: Weak<Composer>,
    this: Weak<Inner<T>>,
}

impl<T> Drop for Inner<T> {
    fn drop(&mut self) {
        if let Some(s) = self.observer.take()
            && let Some(c) = self.composer.upgrade()
        {
            c.remove_observer(s);
            c.set_derived_key(self.key, false);
        }
    }
}

impl<T: Clone + 'static> Inner<T> {
    /// Computes the value again; whether it changed.
    fn recompute(&self) -> bool {
        let (next, dependencies) = match self.observer.get() {
            // Computed at every read: what it reads is its reader's (and dependencies of the one computing).
            None => ((self.calc)(), Default::default()),
            Some(s) => collect_dependencies(|| match self.composer.upgrade() {
                Some(c) => c.observe_into(s, || (self.calc)()),
                None => (self.calc)(),
            }),
        };
        let changed = match &*self.value.borrow() {
            Some(old) => !self.equivalent.is_some_and(|eq| eq(old, &next)),
            None => true,
        };
        let old = self.value.replace(Some(next));
        drop(old);
        self.all_tracked.set(dependencies.all_tracked());
        *self.read_keys.borrow_mut() = dependencies.read.into_iter().collect();
        *self.dependencies.borrow_mut() = dependencies.list;
        self.validated_at.set(Some(read_epoch()));
        self.computations.set(self.computations.get() + 1);
        if changed {
            self.version.set(self.version.get() + 1);
        }
        changed
    }

    /// Whether what it read is no longer what the snapshot read in sees (Compose's `isValid`, negated).
    fn stale(&self) -> bool {
        if self.value.borrow().is_none() {
            return true;
        }
        let epoch = read_epoch();
        if self.validated_at.get() == Some(epoch) {
            return false;
        }
        let computations = self.computations.get();
        let dependencies = self.dependencies.borrow().clone();
        for dependency in &dependencies {
            let changed = dependency.changed();
            if self.computations.get() != computations {
                // Computed meanwhile: a derived state it read changed and told its readers, this one too.
                return false;
            }
            if changed {
                return true;
            }
        }
        self.validated_at.set(Some(read_epoch()));
        false
    }

    /// Its readers told its value changed: those in its composition recompose, the others (a layout's, a
    /// drawing's, a snapshot flow's) are told as a state's are — Compose's reads of a derived state report its
    /// dependencies to them.
    fn tell_readers(&self) {
        if let Some(c) = self.composer.upgrade() {
            c.invalidate_readers(self.key);
        }
        crate::snapshot::state_changed(self.key);
    }

    /// Brought up to date for a read: computed again if what it read changed, its readers told if its value did.
    fn refresh(&self) {
        if self.observer.get().is_none() {
            self.recompute(); // unobserved: always fresh
            return;
        }
        let had_value = self.value.borrow().is_some();
        if self.stale() && self.recompute() && had_value {
            self.tell_readers();
        }
    }

    /// The composition took a change of something it read: computed again unless a read already did since
    /// (every key it read checked by its dependencies, none changed), its readers told if its value changed.
    fn on_inputs_changed(&self) {
        if self.all_tracked.get() && !self.stale() {
            return;
        }
        if self.recompute() {
            self.tell_readers();
        }
    }

    fn read(&self, subscribe: bool) -> std::cell::Ref<'_, T> {
        self.refresh();
        if subscribe {
            if self.observer.get().is_none() {
                tell_observer(self.key);
            } else {
                let this: Weak<dyn DerivedDependency> = self.this.clone();
                if collecting_dependencies() {
                    crate::read::record_dependency(self.key, Dependency::Derived(this.clone(), self.version.get()));
                }
                crate::read::record_derived_read(self.key, this, self.version.get());
            }
        }
        std::cell::Ref::map(self.value.borrow(), |v| v.as_ref().unwrap())
    }
}

/// What a derived state reading this one checks of it: brought up to date, its value's version.
impl<T: Clone + 'static> DerivedDependency for Inner<T> {
    fn validated_version(&self) -> u64 {
        self.refresh();
        self.version.get()
    }

    fn collect_dependency_keys(&self, out: &mut HashSet<StateKey>) {
        out.extend(self.read_keys.borrow().iter().copied());
        let dependencies = self.dependencies.borrow().clone();
        for dependency in dependencies {
            if let Dependency::Derived(derived, _) = dependency
                && let Some(derived) = derived.upgrade()
            {
                derived.collect_dependency_keys(out);
            }
        }
    }
}

impl<T: Clone + 'static> DerivedState<T> {
    pub fn new(calc: impl Fn() -> T + 'static, policy: SnapshotMutationPolicy<T>) -> Self {
        let composer = Composer::current();
        let inner = Rc::new_cyclic(|this: &Weak<Inner<T>>| Inner {
            key: StateKey::new(),
            calc: Box::new(calc),
            equivalent: policy.equivalent,
            value: RefCell::new(None),
            dependencies: RefCell::new(Vec::new()),
            read_keys: RefCell::new(Vec::new()),
            all_tracked: Cell::new(false),
            validated_at: Cell::new(None),
            version: Cell::new(0),
            computations: Cell::new(0),
            observer: Cell::new(None),
            composer: composer.as_ref().map_or_else(Weak::new, Rc::downgrade),
            this: this.clone(),
        });
        if let Some(c) = composer {
            let weak = Rc::downgrade(&inner);
            let s = c.new_observer(Rc::new(move || {
                if let Some(inner) = weak.upgrade() {
                    inner.on_inputs_changed();
                }
            }));
            inner.observer.set(Some(s));
            c.set_derived_key(inner.key, true);
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
