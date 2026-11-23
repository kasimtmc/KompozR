//! lifecycle-runtime's `LifecycleRegistry`: a lifecycle its owner moves, telling its observers each step.

use crate::lifecycle::{
    Lifecycle, LifecycleEvent, LifecycleImpl, LifecycleObserver, LifecycleOwner, LifecycleState, dispatch_to,
    same_observer,
};
use kompoz_coroutines::{MutableStateFlow, StateFlow};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// An observer added, with the state it was last brought to (Kotlin's `ObserverWithState`).
struct ObserverWithState {
    /// Its place in the order observers were added (Kotlin's FastSafeIterableMap keeps that order).
    seq: u64,
    observer: Rc<dyn LifecycleObserver>,
    state: Cell<LifecycleState>,
}

impl ObserverWithState {
    fn dispatch_event(&self, owner: &dyn LifecycleOwner, event: LifecycleEvent) {
        let new_state = event.target_state();
        self.state.set(self.state.get().min(new_state));
        dispatch_to(&*self.observer, owner, event);
        self.state.set(new_state);
    }
}

struct Registry {
    /// The observers in the order they were added; each pass goes by the order, the ones added or removed meanwhile
    /// included or left out (Kotlin's FastSafeIterableMap's iterators).
    observers: RefCell<Vec<Rc<ObserverWithState>>>,
    next_seq: Cell<u64>,
    /// Its owner, held weakly as Kotlin's (gone: an observer added is told nothing, a move is an error).
    owner: Weak<dyn LifecycleOwner>,
    adding_observer_counter: Cell<u32>,
    handling_event: Cell<bool>,
    new_event_occurred: Cell<bool>,
    /// The states of the observers being told an event (an observer added from one doesn't go past them).
    parent_states: RefCell<Vec<LifecycleState>>,
    state: Cell<LifecycleState>,
    state_flow: MutableStateFlow<LifecycleState>,
}

/// A lifecycle its owner moves from state to state (Compose's `LifecycleRegistry`): set its
/// [`current_state`](Self::set_current_state) (or [`handle_lifecycle_event`](Self::handle_lifecycle_event)), and it
/// steps there one event at a time, telling each observer each event in order — up in the order they were added,
/// down in the reverse —; an observer added while it is in a state is told the events up to it at once. It is a
/// [`Lifecycle`] (it derefs to it). Cheap to clone (the same registry).
///
/// ```
/// # use kompoz_lifecycle::*;
/// # use std::rc::{Rc, Weak};
/// struct Screen {
///     registry: LifecycleRegistry,
/// }
/// impl LifecycleOwner for Screen {
///     fn lifecycle(&self) -> Lifecycle {
///         self.registry.lifecycle()
///     }
/// }
/// let screen = Rc::new_cyclic(|me: &Weak<Screen>| Screen { registry: LifecycleRegistry::new(me.clone()) });
/// screen.registry.set_current_state(LifecycleState::Resumed);
/// assert_eq!(screen.lifecycle().current_state(), LifecycleState::Resumed);
/// ```
#[derive(Clone)]
pub struct LifecycleRegistry {
    lifecycle: Lifecycle,
    registry: Rc<Registry>,
}

impl PartialEq for LifecycleRegistry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.registry, &other.registry)
    }
}

impl std::ops::Deref for LifecycleRegistry {
    type Target = Lifecycle;
    fn deref(&self) -> &Lifecycle {
        &self.lifecycle
    }
}

impl From<LifecycleRegistry> for Lifecycle {
    fn from(registry: LifecycleRegistry) -> Lifecycle {
        registry.lifecycle
    }
}

impl LifecycleImpl for Registry {
    fn add_observer(&self, observer: Rc<dyn LifecycleObserver>) {
        let initial = if self.state.get() == LifecycleState::Destroyed {
            LifecycleState::Destroyed
        } else {
            LifecycleState::Initialized
        };
        if self.observers.borrow().iter().any(|o| same_observer(&o.observer, &observer)) {
            return;
        }
        let seq = self.next_seq.get();
        self.next_seq.set(seq + 1);
        let stateful = Rc::new(ObserverWithState { seq, observer: observer.clone(), state: Cell::new(initial) });
        self.observers.borrow_mut().push(stateful.clone());
        let Some(owner) = self.owner.upgrade() else { return }; // (the owner is gone)
        let reentrance = self.adding_observer_counter.get() != 0 || self.handling_event.get();
        let mut target = self.calculate_target_state(seq);
        self.adding_observer_counter.set(self.adding_observer_counter.get() + 1);
        while stateful.state.get() < target && self.contains(seq) {
            self.parent_states.borrow_mut().push(stateful.state.get());
            let event = LifecycleEvent::up_from(stateful.state.get())
                .unwrap_or_else(|| panic!("no event up from {}", stateful.state.get()));
            stateful.dispatch_event(&*owner, event);
            self.parent_states.borrow_mut().pop();
            target = self.calculate_target_state(seq);
        }
        if !reentrance {
            self.sync();
        }
        self.adding_observer_counter.set(self.adding_observer_counter.get() - 1);
    }

    fn remove_observer(&self, observer: Rc<dyn LifecycleObserver>) {
        let removed = {
            let mut observers = self.observers.borrow_mut();
            let at = observers.iter().position(|o| same_observer(&o.observer, &observer));
            at.map(|i| observers.remove(i))
        };
        drop(removed); // (what it holds dropped with nothing of the registry's borrowed)
    }

    fn current_state(&self) -> LifecycleState {
        self.state.get()
    }

    fn current_state_flow(&self) -> Option<StateFlow<LifecycleState>> {
        Some(self.state_flow.as_state_flow())
    }
}

impl Registry {
    fn contains(&self, seq: u64) -> bool {
        self.observers.borrow().iter().any(|o| o.seq == seq)
    }

    /// The observer added before the one of `seq` (Kotlin's `observerMap.ceil(observer)`).
    fn sibling_state(&self, seq: u64) -> Option<LifecycleState> {
        self.observers.borrow().iter().take_while(|o| o.seq < seq).last().map(|o| o.state.get())
    }

    fn calculate_target_state(&self, seq: u64) -> LifecycleState {
        let sibling = self.sibling_state(seq);
        let parent = self.parent_states.borrow().last().copied();
        let mut target = self.state.get();
        for s in [sibling, parent].into_iter().flatten() {
            target = target.min(s);
        }
        target
    }

    fn move_to_state(&self, next: LifecycleState) {
        let current = self.state.get();
        if current == next {
            return;
        }
        check_lifecycle_state_transition(current, next);
        self.state.set(next);
        if self.handling_event.get() || self.adding_observer_counter.get() != 0 {
            self.new_event_occurred.set(true);
            // we will figure out what to do on upper level.
            return;
        }
        self.handling_event.set(true);
        self.sync();
        self.handling_event.set(false);
        if self.state.get() == LifecycleState::Destroyed {
            let observers = std::mem::take(&mut *self.observers.borrow_mut());
            drop(observers);
        }
    }

    fn is_synced(&self) -> bool {
        let observers = self.observers.borrow();
        let (Some(eldest), Some(newest)) = (observers.first(), observers.last()) else { return true };
        let (eldest, newest) = (eldest.state.get(), newest.state.get());
        eldest == newest && self.state.get() == newest
    }

    /// The next observer after `after` (by the order they were added), the ones added meanwhile included.
    fn next_after(&self, after: Option<u64>) -> Option<Rc<ObserverWithState>> {
        self.observers.borrow().iter().find(|o| after.is_none_or(|a| o.seq > a)).cloned()
    }

    /// The next observer before `before`, going back (the ones added meanwhile left out).
    fn next_before(&self, before: u64) -> Option<Rc<ObserverWithState>> {
        self.observers.borrow().iter().rev().find(|o| o.seq < before).cloned()
    }

    fn forward_pass(&self, owner: &dyn LifecycleOwner) {
        let mut at = None;
        while let Some(observer) = self.next_after(at) {
            at = Some(observer.seq);
            while observer.state.get() < self.state.get()
                && !self.new_event_occurred.get()
                && self.contains(observer.seq)
            {
                self.parent_states.borrow_mut().push(observer.state.get());
                let event = LifecycleEvent::up_from(observer.state.get())
                    .unwrap_or_else(|| panic!("no event up from {}", observer.state.get()));
                observer.dispatch_event(owner, event);
                self.parent_states.borrow_mut().pop();
            }
        }
    }

    fn backward_pass(&self, owner: &dyn LifecycleOwner) {
        // (the newest when the pass starts, then back from each)
        let mut at = self.observers.borrow().last().map_or(0, |o| o.seq + 1);
        while let Some(observer) = self.next_before(at) {
            at = observer.seq;
            while observer.state.get() > self.state.get()
                && !self.new_event_occurred.get()
                && self.contains(observer.seq)
            {
                let event = LifecycleEvent::down_from(observer.state.get())
                    .unwrap_or_else(|| panic!("no event down from {}", observer.state.get()));
                self.parent_states.borrow_mut().push(event.target_state());
                observer.dispatch_event(owner, event);
                self.parent_states.borrow_mut().pop();
            }
        }
    }

    /// Brings every observer to the current state: down from the newest, then up from the eldest, again while an
    /// event comes meanwhile.
    fn sync(&self) {
        let owner = self.owner.upgrade().expect(
            "LifecycleOwner of this LifecycleRegistry is already garbage collected. It is too late to change \
             lifecycle state.",
        );
        while !self.is_synced() {
            self.new_event_occurred.set(false);
            let eldest = self.observers.borrow().first().map(|o| o.state.get());
            if eldest.is_some_and(|eldest| self.state.get() < eldest) {
                self.backward_pass(&*owner);
            }
            let newest = self.observers.borrow().last().map(|o| o.state.get());
            if !self.new_event_occurred.get() && newest.is_some_and(|newest| self.state.get() > newest) {
                self.forward_pass(&*owner);
            }
        }
        self.new_event_occurred.set(false);
        self.state_flow.set_value(self.state.get());
    }
}

/// Kotlin's `checkLifecycleStateTransition`: never from `INITIALIZED` straight to `DESTROYED`, never out of
/// `DESTROYED`.
fn check_lifecycle_state_transition(current: LifecycleState, next: LifecycleState) {
    if current == LifecycleState::Initialized && next == LifecycleState::Destroyed {
        panic!("State must be at least '{}' to be moved to '{next}' in component", LifecycleState::Created);
    }
    if current == LifecycleState::Destroyed && current != next {
        panic!("State is '{}' and cannot be moved to `{next}` in component", LifecycleState::Destroyed);
    }
}

impl LifecycleRegistry {
    /// The registry of `provider` (held weakly, as Kotlin's; made with its owner: `Rc::new_cyclic`), in
    /// `INITIALIZED`. (Kotlin checks its calls are on the main thread; a registry is an `Rc` here: it stays on the
    /// thread that made it, which the compiler checks.)
    pub fn new(provider: Weak<dyn LifecycleOwner>) -> LifecycleRegistry {
        LifecycleRegistry::make(provider)
    }

    /// The same (Compose's `createUnsafe`, whose calls aren't checked to be on the main thread: neither are
    /// [`new`](Self::new)'s, kept on their thread by the compiler).
    pub fn create_unsafe(owner: Weak<dyn LifecycleOwner>) -> LifecycleRegistry {
        LifecycleRegistry::make(owner)
    }

    fn make(owner: Weak<dyn LifecycleOwner>) -> LifecycleRegistry {
        let registry = Rc::new(Registry {
            observers: RefCell::new(Vec::new()),
            next_seq: Cell::new(0),
            owner,
            adding_observer_counter: Cell::new(0),
            handling_event: Cell::new(false),
            new_event_occurred: Cell::new(false),
            parent_states: RefCell::new(Vec::new()),
            state: Cell::new(LifecycleState::Initialized),
            state_flow: MutableStateFlow::new(LifecycleState::Initialized),
        });
        let lifecycle = Lifecycle::new(registry.clone());
        LifecycleRegistry { lifecycle, registry }
    }

    /// It as a [`Lifecycle`] (what its owner gives).
    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle.clone()
    }

    /// Moves it to `state`, one event at a time, its observers told each (Kotlin's `currentState = state`).
    ///
    /// # Panics
    /// From `INITIALIZED` to `DESTROYED`, out of `DESTROYED` (Kotlin's IllegalStateException), and with its owner
    /// gone.
    pub fn set_current_state(&self, state: LifecycleState) {
        self.registry.move_to_state(state);
    }

    /// The same (deprecated as Compose's: set [`current_state`](Self::set_current_state)).
    #[deprecated(note = "Override [currentState].")]
    pub fn mark_state(&self, state: LifecycleState) {
        self.set_current_state(state);
    }

    /// Moves it to `event`'s target state (Kotlin's `handleLifecycleEvent`).
    pub fn handle_lifecycle_event(&self, event: LifecycleEvent) {
        self.registry.move_to_state(event.target_state());
    }

    /// How many observers it has.
    pub fn observer_count(&self) -> i32 {
        self.registry.observers.borrow().len() as i32
    }
}
