//! result/ResultEventBus.kt, result/ResultEffect.kt, result/ResultEventBusNavEntryDecorator.kt: results passed
//! between screens.

use super::nav_entry::{NavEntry, NavEntryDecorator};
use super::nav_key::NavEntryKey;
use kompoz_coroutines::{BufferOverflow, Channel, Flow, LaunchedEffect};
use kompoz_runtime::{
    KompozitionLocal, KompozitionLocalProvider, ProvidedValue, SnapshotStateMap, State, kompozable,
    mutable_confined_state_map_of, remember, remember_keyed, remember_updated_state,
};
use std::any::Any;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// The key of results of type `T` (Kotlin's `T::class.toString()`: `"class " + its name`).
fn class_key<T: 'static>() -> String {
    format!("class {}", std::any::type_name::<T>())
}

/// What passes results between screens (Compose's `ResultEventBus`): a channel of results per key. An event result
/// is received by a [`ResultEffect`] (each one), a state result by [`conflate_as_state`](Self::conflate_as_state) (the
/// latest of a key). Results aren't saved (configuration changes, process death). Clones are the same bus.
#[derive(Clone)]
pub struct ResultEventBus {
    /// A channel of results per key (Compose's internal `channelMap`).
    pub(crate) channel_map: SnapshotStateMap<String, Channel<Rc<dyn Any>>>,
}

impl PartialEq for ResultEventBus {
    fn eq(&self, other: &Self) -> bool {
        self.channel_map == other.channel_map
    }
}

impl Default for ResultEventBus {
    fn default() -> Self {
        ResultEventBus::new()
    }
}

/// A new bus (Compose's constructor).
///
/// Kotlin: `ResultEventBus()`
pub fn ResultEventBus() -> ResultEventBus {
    ResultEventBus::new()
}

impl ResultEventBus {
    pub fn new() -> ResultEventBus {
        ResultEventBus { channel_map: mutable_confined_state_map_of([]) }
    }

    fn channel(&self, result_key: &str) -> Channel<Rc<dyn Any>> {
        if let Some(c) = self.channel_map.get(&result_key.to_string()) {
            return c;
        }
        let c = Channel::new(Channel::<Rc<dyn Any>>::BUFFERED, BufferOverflow::Suspend);
        self.channel_map.put(result_key.to_string(), c.clone());
        c
    }

    /// The channel of `result_key` now, if it has one (what a [`ResultEffect`] is keyed by).
    pub(crate) fn channel_of(&self, result_key: &str) -> Option<Channel<Rc<dyn Any>>> {
        self.channel_map.get(&result_key.to_string())
    }

    /// The results of `result_key` (Compose's internal `getResultFlow`): its channel's, made if it has none.
    pub fn get_result_flow(&self, result_key: &str) -> Flow<Rc<dyn Any>> {
        self.channel(result_key).receive_as_flow()
    }

    /// The latest result of type `T` as a state (Compose's reified `conflateAsState<T>(defaultValue)`): its key
    /// `T`'s class. Composable.
    ///
    /// Kotlin: `conflateAsState(defaultValue: T): State<T>`
    #[track_caller]
    pub fn conflate_as_state<T: Clone + PartialEq + 'static>(&self, default_value: T) -> State<T> {
        self.conflate_as_state_of_key(&class_key::<T>(), default_value)
    }

    /// The latest result of `result_key` as a state (Compose's `conflateAsState(resultKey, defaultValue)`),
    /// `default_value` until there is one. Composable.
    ///
    /// Kotlin: `conflateAsState(resultKey: String, defaultValue: T): State<T>`
    #[track_caller]
    pub fn conflate_as_state_of_key<T: Clone + PartialEq + 'static>(
        &self,
        result_key: &str,
        default_value: T,
    ) -> State<T> {
        kompoz_runtime::group(|| {
            let bus = self.clone();
            let key = result_key.to_string();
            // (Compose's `remember(resultKey) { getResultFlow(resultKey) }`, its values as `T`: Kotlin's unchecked
            // `as State<T>`, a result of another type a ClassCastException where it is read — here as it comes)
            let flow = remember_keyed(key.clone(), move || {
                bus.get_result_flow(&key).map(|v: Rc<dyn Any>| match v.downcast_ref::<T>() {
                    Some(v) => v.clone(),
                    None => kompoz_runtime::kt::panic_kt(
                        "ClassCastException",
                        &format!("a result cannot be cast to {}", std::any::type_name::<T>()),
                    ),
                })
            });
            // (Compose's `flow.collectAsState(defaultValue)`: results needn't be Send, a confined state)
            flow.collect_as_confined_state(default_value)
        })
    }

    /// Sends `result` with the key of its type (Compose's reified `sendResult<T>(result)`).
    ///
    /// Kotlin: `sendResult(result: T)`
    pub fn send_result<T: 'static>(&self, result: T) {
        self.send_result_of_key(&class_key::<T>(), result);
    }

    /// Sends `result` into `result_key`'s channel (Compose's `sendResult(resultKey, result)`), made if it has none.
    ///
    /// Kotlin: `sendResult(resultKey: String, result: T)`
    pub fn send_result_of_key<T: 'static>(&self, result_key: &str, result: T) {
        self.channel(result_key).try_send(Rc::new(result));
    }

    /// Removes the results of type `T` and stops giving them (Compose's reified `removeResult<T>()`).
    ///
    /// Kotlin: `removeResult()`
    pub fn remove_result<T: 'static>(&self) {
        self.remove_result_of_key(&class_key::<T>());
    }

    /// Removes the results of `result_key` and stops giving them (Compose's `removeResult(resultKey)`): its channel
    /// closed.
    ///
    /// Kotlin: `removeResult(resultKey: String)`
    pub fn remove_result_of_key(&self, result_key: &str) {
        if let Some(c) = self.channel_map.remove(&result_key.to_string()) {
            c.close();
        }
    }
}

/// A [`ResultEventBus`] remembered across recompositions (Compose's `rememberResultEventBus`).
#[kompozable]
pub fn remember_result_event_bus() -> ResultEventBus {
    remember(ResultEventBus::new)
}

/// The composition local of the [`ResultEventBus`] results are received from (Compose's object
/// `LocalResultEventBus`): its `current` panics where none is provided.
pub struct LocalResultEventBusLocal {
    local: KompozitionLocal<Option<ResultEventBus>>,
}

/// Kotlin: `LocalResultEventBus`
pub static LocalResultEventBus: LocalResultEventBusLocal =
    LocalResultEventBusLocal { local: KompozitionLocal::new(|| None) };

impl LocalResultEventBusLocal {
    /// The bus provided here (Compose's `current`).
    ///
    /// # Panics
    /// "No ResultEventBus has been provided".
    #[track_caller]
    pub fn current(&'static self) -> ResultEventBus {
        self.local.current().unwrap_or_else(|| kompoz_runtime::kt::error("No ResultEventBus has been provided"))
    }

    /// `event_bus` provided below (Kotlin's infix `provides`).
    pub fn provides(&'static self, event_bus: ResultEventBus) -> ProvidedValue {
        self.local.provides(Some(event_bus))
    }
}

/// Receives each result of type `T` (Compose's reified `ResultEffect<T>(resultEventBus, onResult)`): `on_result` (the
/// latest given) runs for each, in a coroutine. Composable.
///
/// Kotlin: `ResultEffect(resultEventBus: ResultEventBus = LocalResultEventBus.current, onResult: suspend (T) -> Unit)`
#[kompozable(inline)]
pub fn ResultEffect<T: Clone + 'static>(
    #[default(LocalResultEventBus.current())] result_event_bus: ResultEventBus,
    on_result: impl ResultHandler<T>,
) {
    result_effect(class_key::<T>(), result_event_bus, Rc::new(on_result));
}

/// Receives each result of `result_key` (Compose's `ResultEffect(resultKey, resultEventBus, onResult)`): `on_result`
/// (the latest given) runs for each, in a coroutine, from the key's channel (again when it is replaced). Composable.
///
/// Kotlin: `ResultEffect(resultKey: String, resultEventBus: ResultEventBus = LocalResultEventBus.current, onResult: suspend (T) -> Unit)`
#[kompozable(inline)]
pub fn ResultEffectOfKey<T: Clone + 'static>(
    result_key: String,
    #[default(LocalResultEventBus.current())] result_event_bus: ResultEventBus,
    on_result: impl ResultHandler<T>,
) {
    result_effect(result_key, result_event_bus, Rc::new(on_result));
}

/// What a [`ResultEffect`] does with a result (Kotlin's `onResult: suspend (T) -> Unit`): a suspending lambda of
/// the result, `|result: T| async move { … }` (its parameter's type the results').
pub trait ResultHandler<T>: 'static {
    /// The coroutine handling `result`.
    fn handle(&self, result: T) -> Pin<Box<dyn Future<Output = ()>>>;
}

impl<T, Fut: Future<Output = ()> + 'static, F: Fn(T) -> Fut + 'static> ResultHandler<T> for F {
    fn handle(&self, result: T) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(self(result))
    }
}

/// The body of both `ResultEffect`s.
#[track_caller]
fn result_effect<T: Clone + 'static>(
    result_key: String,
    result_event_bus: ResultEventBus,
    on_result: Rc<dyn ResultHandler<T>>,
) {
    let current_on_result = remember_updated_state!(on_result);
    let channel = result_event_bus.channel_of(&result_key);
    let (bus, key) = (result_event_bus.clone(), result_key.clone());
    kompoz_runtime::kompozable_block!({
        LaunchedEffect((result_key, channel), async move {
            let flow = bus.get_result_flow(&key);
            flow.collect_async(move |result: Rc<dyn Any>| {
                let current = current_on_result.get();
                let value = result.downcast_ref::<T>().cloned();
                async move {
                    if let Some(v) = value {
                        current.handle(v).await;
                    }
                }
            })
            .await;
        });
    });
}

/// What [`ResultEventBusNavEntryDecorator`] makes (Compose's class of that name).
pub type ResultEventBusNavEntryDecorator<T> = NavEntryDecorator<T>;

/// The decorator providing `bus` as [`LocalResultEventBus`] to each entry's content (Compose's
/// `ResultEventBusNavEntryDecorator`): results passed to entries below on the back stack.
///
/// Kotlin: `ResultEventBusNavEntryDecorator(bus: ResultEventBus = ResultEventBus())`
pub fn ResultEventBusNavEntryDecorator<T: NavEntryKey>(bus: ResultEventBus) -> NavEntryDecorator<T> {
    NavEntryDecorator::new(
        |_| {},
        move |entry: NavEntry<T>| {
            kompoz_runtime::kompozable_block!({
                KompozitionLocalProvider(&[LocalResultEventBus.provides(bus.clone())], || entry.Content());
            });
        },
    )
}

/// A [`ResultEventBusNavEntryDecorator`] of `result_event_bus` remembered across recompositions (Compose's
/// `rememberResultEventBusNavEntryDecorator`; its first bus kept, as Compose's `remember { }` without keys).
///
/// Kotlin: `rememberResultEventBusNavEntryDecorator(resultEventBus: ResultEventBus = rememberResultEventBus()): ResultEventBusNavEntryDecorator<T>`
#[kompozable]
pub fn remember_result_event_bus_nav_entry_decorator<T: NavEntryKey>(
    #[default(remember_result_event_bus())] result_event_bus: ResultEventBus,
) -> NavEntryDecorator<T> {
    remember(move || ResultEventBusNavEntryDecorator(result_event_bus))
}
