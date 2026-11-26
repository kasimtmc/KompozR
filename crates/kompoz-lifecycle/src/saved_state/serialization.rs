//! savedstate's serialization (SavedStateEncoder/Decoder's `encodeToSavedState`/`decodeFromSavedState`,
//! SavedStateRegistryOwnerDelegate's `saved { }`) over KompozR's [`Saveable`] (`#[derive(Saveable)]`, Kotlin's
//! `@Serializable`) and [`Saver`] (Kotlin's explicit `KSerializer`): kotlinx.serialization has no Rust form.

use super::{SavedState, SavedStateProvider, SavedStateRegistryOwner};
use kompoz_runtime::kt::{KtDelegate, KtMutableDelegate};
use kompoz_runtime::{Saveable, SavedValue, Saver};
use std::cell::RefCell;
use std::rc::Rc;

/// `value` as a saved state (Compose's `encodeToSavedState(value)`): a value saved as a map (a struct of
/// `#[derive(Saveable)]`, a saved state) is its entries, by field; any other is under the key `""` (as
/// kotlinx.serialization writes a value that isn't a structure).
///
/// ```
/// # use kompoz_lifecycle::*;
/// # use kompoz_runtime::Saveable;
/// #[derive(Saveable, PartialEq, Debug)]
/// struct User {
///     id: i32,
///     name: String,
/// }
/// let saved = encode_to_saved_state(&User { id: 123, name: "foo".into() });
/// assert_eq!(saved.read(|r| r.get_int("id")), 123);
/// assert_eq!(decode_from_saved_state::<User>(&saved), User { id: 123, name: "foo".into() });
/// ```
pub fn encode_to_saved_state<T: Saveable>(value: &T) -> SavedState {
    from_saved_value(value.save())
}

/// `value` as a saved state, written by `serializer` (Compose's `encodeToSavedState(serializer, value)`).
///
/// Kotlin: `encodeToSavedState(serializer, value)`
pub fn encode_to_saved_state_with<T: 'static>(serializer: &Saver<T>, value: &T) -> SavedState {
    from_saved_value(serializer.save(value))
}

/// `value` as a saved state, written by `serializer` with `configuration` in effect (Compose's
/// `encodeToSavedState(serializer, value, configuration)`).
///
/// Kotlin: `encodeToSavedState(serializer: SerializationStrategy<T>, value: T, configuration: SavedStateConfiguration)`
pub fn encode_to_saved_state_configured<T: 'static>(
    serializer: &Saver<T>,
    value: &T,
    configuration: &super::SavedStateConfiguration,
) -> SavedState {
    configuration.in_effect(|| encode_to_saved_state_with(serializer, value))
}

fn from_saved_value(saved: SavedValue) -> SavedState {
    if saved.as_map().is_some() {
        SavedState::from_saved_value(&saved)
    } else {
        super::saved_state(|w| w.put_value("", saved))
    }
}

/// The value `saved_state` holds, as [`encode_to_saved_state`] wrote it (Compose's `decodeFromSavedState<T>`).
///
/// # Panics
/// When it doesn't hold a `T` (Kotlin's SerializationException).
#[track_caller]
pub fn decode_from_saved_state<T: Saveable>(saved_state: &SavedState) -> T {
    decode(saved_state, T::restore)
}

/// The value `saved_state` holds, read by `deserializer` (Compose's `decodeFromSavedState(deserializer,
/// savedState)`).
///
/// Kotlin: `decodeFromSavedState(deserializer, savedState)`
#[track_caller]
pub fn decode_from_saved_state_with<T: 'static>(deserializer: &Saver<T>, saved_state: &SavedState) -> T {
    decode(saved_state, |v| deserializer.restore(v))
}

/// The value `saved_state` holds, read by `deserializer` with `configuration` in effect (Compose's
/// `decodeFromSavedState(deserializer, savedState, configuration)`): a polymorphic value's subclass found by the
/// configuration's serializers module.
///
/// Kotlin: `decodeFromSavedState(deserializer: DeserializationStrategy<T>, savedState: SavedState, configuration: SavedStateConfiguration)`
#[track_caller]
pub fn decode_from_saved_state_configured<T: 'static>(
    deserializer: &Saver<T>,
    saved_state: &SavedState,
    configuration: &super::SavedStateConfiguration,
) -> T {
    configuration.in_effect(|| decode_from_saved_state_with(deserializer, saved_state))
}

#[track_caller]
fn decode<T>(saved_state: &SavedState, restore: impl Fn(&SavedValue) -> Option<T>) -> T {
    let whole = saved_state.to_saved_value();
    restore(&whole).or_else(|| saved_state.read(|r| r.get_value("")).and_then(|v| restore(&v))).unwrap_or_else(|| {
        panic!("SerializationException: {} can't be decoded from {saved_state:?}", std::any::type_name::<T>())
    })
}

/// What a `saved` delegate is (Compose's private SavedStateRegistryOwnerDelegate / SavedStateHandleDelegate, a
/// `ReadWriteProperty`): a value saved under its key with its owner's state and restored from it. Its first read
/// registers it and loads the value (restored, else `init()`'s); its first write registers it and sets it. Read and
/// written as a Kotlin `by` delegate ([`get_value`](KtDelegate::get_value) / [`set_value`](KtMutableDelegate::
/// set_value)); a clone is the same delegate.
pub struct SavedStateDelegate<T: 'static> {
    inner: Rc<DelegateInner<T>>,
}

impl<T: 'static> Clone for SavedStateDelegate<T> {
    fn clone(&self) -> Self {
        SavedStateDelegate { inner: self.inner.clone() }
    }
}

type Register = Box<dyn Fn(&str, Rc<dyn SavedStateProvider>)>;
type Load = Box<dyn Fn(&str) -> Option<SavedState>>;

struct DelegateInner<T: 'static> {
    key: String,
    serializer: Saver<T>,
    init: RefCell<Option<Box<dyn FnOnce() -> T>>>,
    /// None: not read or written yet (Kotlin's `UNINITIALIZED`).
    cached: RefCell<Option<T>>,
    register: Register,
    load: Load,
}

impl<T: 'static> SavedStateProvider for DelegateInner<T> {
    fn save_state(&self) -> SavedState {
        match &*self.cached.borrow() {
            // (never read: nothing saved)
            None => SavedState::new(),
            Some(value) => encode_to_saved_state_with(&self.serializer, value),
        }
    }
}

impl<T: 'static> SavedStateDelegate<T> {
    /// A delegate saving its value under `key` through `register` (a registry's, a handle's), loading what was
    /// saved through `load`.
    pub(crate) fn new(
        key: String,
        serializer: Saver<T>,
        init: impl FnOnce() -> T + 'static,
        register: impl Fn(&str, Rc<dyn SavedStateProvider>) + 'static,
        load: impl Fn(&str) -> Option<SavedState> + 'static,
    ) -> SavedStateDelegate<T> {
        SavedStateDelegate {
            inner: Rc::new(DelegateInner {
                key,
                serializer,
                init: RefCell::new(Some(Box::new(init))),
                cached: RefCell::new(None),
                register: Box::new(register),
                load: Box::new(load),
            }),
        }
    }

    fn provider(&self) -> Rc<dyn SavedStateProvider> {
        self.inner.clone()
    }
}

impl<T: Clone + 'static> KtDelegate for SavedStateDelegate<T> {
    type Value = T;
    fn get_value(&self) -> T {
        let inner = &self.inner;
        if inner.cached.borrow().is_none() {
            (inner.register)(&inner.key, self.provider());
            let value = match (inner.load)(&inner.key) {
                Some(restored) => decode_from_saved_state_with(&inner.serializer, &restored),
                None => (inner.init.borrow_mut().take().expect("kompoz: a saved delegate initialized once"))(),
            };
            *inner.cached.borrow_mut() = Some(value);
        }
        inner.cached.borrow().clone().expect("kompoz: the value is there")
    }
}

impl<T: Clone + 'static> KtMutableDelegate for SavedStateDelegate<T> {
    fn set_value(&self, value: T) {
        let inner = &self.inner;
        if inner.cached.borrow().is_none() {
            (inner.register)(&inner.key, self.provider());
        }
        *inner.cached.borrow_mut() = Some(value);
    }
}

/// savedstate's `SavedStateRegistryOwner.saved { }` (SavedStateRegistryOwnerDelegate.kt), for every owner.
pub trait SavedStateRegistryOwnerExtensions: SavedStateRegistryOwner {
    /// A value saved with the owner's state under `key` (Compose's `val user by saved(key) { User(…) }`): read, it
    /// is what was saved before the process ended, else `init()`'s. (Kotlin's key defaults to the property's
    /// qualified name, which Rust can't name: it is given.)
    ///
    /// Kotlin: `saved(key: String, init: () -> T)`
    fn saved<T: Saveable + Clone + 'static>(
        &self,
        key: &str,
        init: impl FnOnce() -> T + 'static,
    ) -> SavedStateDelegate<T> {
        self.saved_with(Saver::new(|v: &T| v.save(), T::restore), key, init)
    }

    /// The same, written by `serializer` (Compose's `saved(serializer, key) { … }`).
    ///
    /// Kotlin: `saved(serializer: KSerializer<T>, key: String, init: () -> T)`
    fn saved_with<T: Clone + 'static>(
        &self,
        serializer: Saver<T>,
        key: &str,
        init: impl FnOnce() -> T + 'static,
    ) -> SavedStateDelegate<T> {
        let registry = self.saved_state_registry();
        let consume = registry.clone();
        SavedStateDelegate::new(
            key.to_string(),
            serializer,
            init,
            move |key, provider| registry.register_saved_state_provider(key, provider),
            move |key| consume.consume_restored_state_for_key(key),
        )
    }
}

impl<O: SavedStateRegistryOwner + ?Sized> SavedStateRegistryOwnerExtensions for O {}
