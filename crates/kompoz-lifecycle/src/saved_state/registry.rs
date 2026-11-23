//! savedstate's SavedStateRegistry.kt, SavedStateRegistryImpl.kt, SavedStateRegistryOwner.kt and
//! SavedStateRegistryController.kt.

use super::{SavedState, saved_state};
use crate::lifecycle::{LifecycleEvent, LifecycleObserver, LifecycleOwner, LifecycleState};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// What a component saves its state with (Compose's `SavedStateRegistry.SavedStateProvider`, a fun interface): asked
/// as its owner's state is saved, what it gives is restored under its key ([`SavedStateRegistry::
/// consume_restored_state_for_key`]). A closure `|| saved_state(…)` is one.
pub trait SavedStateProvider: Any {
    /// The state to save (Kotlin's `saveState()`).
    fn save_state(&self) -> SavedState;
}

impl<F: Fn() -> SavedState + 'static> SavedStateProvider for F {
    fn save_state(&self) -> SavedState {
        self()
    }
}

/// Where an owner's components register what they save and consume what they saved before (Compose's
/// `SavedStateRegistry`): an activity's, a navigation entry's, the host's. Made by its owner's
/// [`SavedStateRegistryController`]; a clone is the same registry.
#[derive(Clone)]
pub struct SavedStateRegistry(Rc<RegistryImpl>);

impl PartialEq for SavedStateRegistry {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Compose's `SavedStateRegistryImpl`.
struct RegistryImpl {
    owner: Weak<dyn SavedStateRegistryOwner>,
    /// By key, in the order registered (Kotlin's `mutableMapOf`, a LinkedHashMap).
    key_to_providers: RefCell<Vec<(String, Rc<dyn SavedStateProvider>)>>,
    attached: Cell<bool>,
    restored_state: RefCell<Option<SavedState>>,
    is_restored: Cell<bool>,
    is_allowing_saving_state: Cell<bool>,
}

/// Where a registry keeps its components' states in its owner's (Compose's `SAVED_COMPONENTS_KEY`).
const SAVED_COMPONENTS_KEY: &str = "androidx.lifecycle.BundlableSavedStateRegistry.key";

impl SavedStateRegistry {
    /// Whether its owner restored it (with a state or none): only then can what was saved be consumed.
    pub fn is_restored(&self) -> bool {
        self.0.is_restored.get()
    }

    /// The state the provider of `key` saved, given once (a second call gives none) (Compose's
    /// `consumeRestoredStateForKey`); what isn't consumed is saved again with the owner's state.
    ///
    /// # Panics
    /// Before its owner restored it ("You can 'consumeRestoredStateForKey' only after the corresponding component
    /// has moved to the 'CREATED' state").
    #[track_caller]
    pub fn consume_restored_state_for_key(&self, key: &str) -> Option<SavedState> {
        assert!(
            self.0.is_restored.get(),
            "IllegalStateException: You can 'consumeRestoredStateForKey' only after the corresponding component has \
             moved to the 'CREATED' state"
        );
        let state = self.0.restored_state.borrow().clone()?;
        let consumed = state.read(|r| if r.contains(key) { r.get_saved_state_or_null(key) } else { None });
        state.write(|w| w.remove(key));
        if state.read(|r| r.is_empty()) {
            *self.0.restored_state.borrow_mut() = None;
        }
        consumed
    }

    /// Asks `provider` for its state under `key` whenever the owner's is saved (Compose's
    /// `registerSavedStateProvider`); it replaces what was restored and not consumed under that key.
    ///
    /// # Panics
    /// If a provider is registered under `key` already ("SavedStateProvider with the given key is already
    /// registered").
    #[track_caller]
    pub fn register_saved_state_provider(&self, key: &str, provider: Rc<dyn SavedStateProvider>) {
        let mut providers = self.0.key_to_providers.borrow_mut();
        assert!(
            !providers.iter().any(|(k, _)| k == key),
            "IllegalArgumentException: SavedStateProvider with the given key is already registered"
        );
        providers.push((key.to_string(), provider));
    }

    /// The provider registered under `key`, if one is (Compose's `getSavedStateProvider`).
    pub fn get_saved_state_provider(&self, key: &str) -> Option<Rc<dyn SavedStateProvider>> {
        self.0.key_to_providers.borrow().iter().find(|(k, _)| k == key).map(|(_, p)| p.clone())
    }

    /// No longer asks the provider of `key` (Compose's `unregisterSavedStateProvider`).
    pub fn unregister_saved_state_provider(&self, key: &str) {
        let removed = {
            let mut providers = self.0.key_to_providers.borrow_mut();
            providers.iter().position(|(k, _)| k == key).map(|i| providers.remove(i))
        };
        drop(removed);
    }

    /// Whether its owner may save now (started; not between ON_STOP and ON_START: Compose's internal
    /// `isAllowingSavingState`).
    pub fn is_allowing_saving_state(&self) -> bool {
        self.0.is_allowing_saving_state.get()
    }

    fn owner(&self) -> Rc<dyn SavedStateRegistryOwner> {
        self.0.owner.upgrade().expect("kompoz: a saved state registry's owner is gone")
    }

    fn perform_attach(&self) {
        let owner = self.owner();
        assert!(
            owner.lifecycle().current_state() == LifecycleState::Initialized,
            "IllegalStateException: Restarter must be created only during owner's initialization stage"
        );
        assert!(!self.0.attached.get(), "IllegalStateException: SavedStateRegistry was already attached.");
        let me = Rc::downgrade(&self.0);
        let observer: Rc<dyn LifecycleObserver> = Rc::new(move |_: &dyn LifecycleOwner, event: LifecycleEvent| {
            if let Some(me) = me.upgrade() {
                if event == LifecycleEvent::OnStart {
                    me.is_allowing_saving_state.set(true);
                } else if event == LifecycleEvent::OnStop {
                    me.is_allowing_saving_state.set(false);
                }
            }
        });
        owner.lifecycle().add_observer(observer);
        self.0.attached.set(true);
    }

    fn perform_restore(&self, saved_state: Option<&SavedState>) {
        if !self.0.attached.get() {
            self.perform_attach();
        }
        let state = self.owner().lifecycle().current_state();
        assert!(
            !state.is_at_least(LifecycleState::Started),
            "IllegalStateException: performRestore cannot be called when owner is {state}"
        );
        assert!(!self.0.is_restored.get(), "IllegalStateException: SavedStateRegistry was already restored.");
        *self.0.restored_state.borrow_mut() = saved_state.and_then(|s| {
            s.read(|r| {
                if r.contains(SAVED_COMPONENTS_KEY) { r.get_saved_state_or_null(SAVED_COMPONENTS_KEY) } else { None }
            })
        });
        self.0.is_restored.set(true);
    }

    fn perform_save(&self, out_bundle: &SavedState) {
        let restored = self.0.restored_state.borrow().clone();
        let providers = self.0.key_to_providers.borrow().clone();
        let in_state = saved_state(|w| {
            if let Some(r) = &restored {
                w.put_all(r);
            }
            for (key, provider) in &providers {
                w.put_saved_state(key, &provider.save_state());
            }
        });
        if !in_state.read(|r| r.is_empty()) {
            out_bundle.write(|w| w.put_saved_state(SAVED_COMPONENTS_KEY, &in_state));
        }
    }
}

/// What owns a [`SavedStateRegistry`] (Compose's `SavedStateRegistryOwner`, a [`LifecycleOwner`]): the host, a
/// navigation entry. Its registry is made by a [`SavedStateRegistryController`] as the owner is made (in
/// `INITIALIZED`), restored before it is started, saved after it is stopped. Shared as
/// `Rc<dyn SavedStateRegistryOwner>`, told apart by identity.
pub trait SavedStateRegistryOwner: LifecycleOwner {
    /// Its registry.
    fn saved_state_registry(&self) -> SavedStateRegistry;
}

impl PartialEq for dyn SavedStateRegistryOwner {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// What a [`SavedStateRegistryOwner`] controls its registry with (Compose's `SavedStateRegistryController`):
/// made as the owner is (`Rc::new_cyclic`, the owner held weakly), [`perform_attach`](Self::perform_attach)ed in
/// `INITIALIZED`, [`perform_restore`](Self::perform_restore)d before the owner is started,
/// [`perform_save`](Self::perform_save)d into the owner's saved state.
///
/// ```
/// # use kompoz_lifecycle::*;
/// # use std::rc::{Rc, Weak};
/// struct Screen {
///     lifecycle: LifecycleRegistry,
///     controller: SavedStateRegistryController,
/// }
/// impl LifecycleOwner for Screen {
///     fn lifecycle(&self) -> Lifecycle { self.lifecycle.lifecycle() }
/// }
/// impl SavedStateRegistryOwner for Screen {
///     fn saved_state_registry(&self) -> SavedStateRegistry { self.controller.saved_state_registry() }
/// }
/// let screen = Rc::new_cyclic(|me: &Weak<Screen>| Screen {
///     lifecycle: LifecycleRegistry::new(me.clone()),
///     controller: SavedStateRegistryController::create(me.clone()),
/// });
/// screen.controller.perform_attach();
/// screen.controller.perform_restore(None);
/// screen.lifecycle.set_current_state(LifecycleState::Resumed);
/// ```
pub struct SavedStateRegistryController {
    registry: SavedStateRegistry,
}

impl SavedStateRegistryController {
    /// The controller of `owner`'s registry (Compose's `create(owner)`), made with the owner.
    pub fn create(owner: Weak<dyn SavedStateRegistryOwner>) -> SavedStateRegistryController {
        SavedStateRegistryController {
            registry: SavedStateRegistry(Rc::new(RegistryImpl {
                owner,
                key_to_providers: RefCell::new(Vec::new()),
                attached: Cell::new(false),
                restored_state: RefCell::new(None),
                is_restored: Cell::new(false),
                is_allowing_saving_state: Cell::new(true),
            })),
        }
    }

    /// Its registry.
    pub fn saved_state_registry(&self) -> SavedStateRegistry {
        self.registry.clone()
    }

    /// Attaches the registry to its owner's lifecycle, once, while the owner is `INITIALIZED`.
    ///
    /// # Panics
    /// When the owner isn't `INITIALIZED`, or a second time.
    #[track_caller]
    pub fn perform_attach(&self) {
        self.registry.perform_attach();
    }

    /// Restores the registry from `saved_state` (what [`perform_save`](Self::perform_save) wrote; none: nothing),
    /// once, before the owner is started (attaching it first if it isn't).
    ///
    /// # Panics
    /// When the owner is started, or a second time.
    #[track_caller]
    pub fn perform_restore(&self, saved_state: Option<&SavedState>) {
        self.registry.perform_restore(saved_state);
    }

    /// Writes into `out_bundle` the states of the registry's providers and what was restored and not consumed.
    pub fn perform_save(&self, out_bundle: &SavedState) {
        self.registry.perform_save(out_bundle);
    }
}
