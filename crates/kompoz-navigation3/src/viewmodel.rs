//! lifecycle-viewmodel-navigation3 2.11 (androidx.lifecycle.viewmodel.navigation3): ViewModelStoreNavEntryDecorator.kt.

use crate::runtime::{NavEntry, NavEntryDecorator, NavEntryKey, saveable_key};
use kompoz_lifecycle::{
    AnyKey, LocalSavedStateRegistryOwner, LocalViewModelStoreOwner, SavedState, ViewModelStore, ViewModelStoreOwner,
    ViewModelStoreProvider, remember_view_model_store_owner_keyed, remember_view_model_store_provider,
};
use kompoz_runtime::{KompozitionLocalProvider, kompozable, remember_keyed};
use kompoz_ui::layout::SlotKey;
use std::rc::Rc;

/// The key of an entry's ViewModel store in the provider: its content key (a text or a number).
fn store_key(content_key: &SlotKey) -> AnyKey {
    AnyKey::String(saveable_key(content_key))
}

/// What [`ViewModelStoreNavEntryDecorator`] makes (Compose's class of that name, a `NavEntryDecorator`).
pub type ViewModelStoreNavEntryDecorator<T> = NavEntryDecorator<T>;

/// The decorator giving each entry a ViewModel store owner of its own (Compose's `ViewModelStoreNavEntryDecorator`):
/// `view_model_store_provider`'s store of the entry's content key, provided as [`LocalViewModelStoreOwner`] to its
/// content (its ViewModels' handles saved with the entry's `LocalSavedStateRegistryOwner`); the store cleared when
/// the entry's content key is popped. Used with the saveable state holder's decorator (the entry's saved state).
///
/// Kotlin: `ViewModelStoreNavEntryDecorator(viewModelStoreProvider: ViewModelStoreProvider)`
pub fn ViewModelStoreNavEntryDecorator<T: NavEntryKey>(
    view_model_store_provider: ViewModelStoreProvider,
) -> NavEntryDecorator<T> {
    let (popped, provider) = (view_model_store_provider.clone(), view_model_store_provider);
    NavEntryDecorator::new(
        move |content_key: SlotKey| popped.clear_key(store_key(&content_key)),
        move |entry: NavEntry<T>| {
            let provider = provider.clone();
            let key = store_key(&entry.content_key());
            kompoz_runtime::kompozable_block!({
                let owner = remember_view_model_store_owner_keyed(
                    key,
                    provider,
                    saved_state_registry_owner = Some(LocalSavedStateRegistryOwner.current()),
                );
                KompozitionLocalProvider(&[LocalViewModelStoreOwner.provides(owner)], || entry.Content());
            });
        },
    )
}

/// The decorator of a store of its own, kept in `view_model_store` (Compose's deprecated constructor
/// `ViewModelStoreNavEntryDecorator(viewModelStore, removeViewModelStoreOnPop)`: the lambda is never called — all
/// decorator state is cleared on pop).
///
/// Kotlin: `ViewModelStoreNavEntryDecorator(viewModelStore: ViewModelStore, removeViewModelStoreOnPop: () -> Boolean)`
#[deprecated(note = "This parameter was a workaround for detecting configuration changes and was never intended for \
                     conditional popping. Use ViewModelStoreNavEntryDecorator(viewModelStoreProvider).")]
pub fn ViewModelStoreNavEntryDecorator_of_store<T: NavEntryKey>(
    view_model_store: ViewModelStore,
    _remove_view_model_store_on_pop: impl Fn() -> bool + 'static,
) -> NavEntryDecorator<T> {
    ViewModelStoreNavEntryDecorator(ViewModelStoreProvider::new(
        Some(view_model_store),
        "androidx.lifecycle.viewmodel.navigation3.ViewModelStoreNavEntryDecorator",
        SavedState::new(),
        None,
        None,
    ))
}

/// The nearest ViewModel store owner, which must be there (Kotlin's `checkNotNull(LocalViewModelStoreOwner.current)`).
#[doc(hidden)]
#[track_caller]
pub fn required_view_model_store_owner() -> Rc<dyn ViewModelStoreOwner> {
    LocalViewModelStoreOwner.current().unwrap_or_else(|| {
        kompoz_runtime::kt::panic_kt(
            "IllegalStateException",
            "No ViewModelStoreOwner was provided via LocalViewModelStoreOwner",
        )
    })
}

/// A [`ViewModelStoreNavEntryDecorator`] remembered across recompositions, its stores in a provider kept in
/// `view_model_store_owner`'s store (by default the nearest owner's) (Compose's
/// `rememberViewModelStoreNavEntryDecorator(viewModelStoreOwner)`): its default factory and creation extras the
/// owner's.
///
/// Kotlin: `rememberViewModelStoreNavEntryDecorator(viewModelStoreOwner: ViewModelStoreOwner = checkNotNull(LocalViewModelStoreOwner.current) { "No ViewModelStoreOwner was provided via LocalViewModelStoreOwner" }): ViewModelStoreNavEntryDecorator<T>`
#[kompozable]
pub fn remember_view_model_store_nav_entry_decorator<T: NavEntryKey>(
    #[default(required_view_model_store_owner())] view_model_store_owner: Rc<dyn ViewModelStoreOwner>,
) -> NavEntryDecorator<T> {
    let provider = remember_view_model_store_provider(parent = Some(view_model_store_owner));
    remember_view_model_store_nav_entry_decorator_of(provider)
}

/// A [`ViewModelStoreNavEntryDecorator`] of `view_model_store_provider` remembered across recompositions (Compose's
/// `rememberViewModelStoreNavEntryDecorator(viewModelStoreProvider)`).
///
/// Kotlin: `rememberViewModelStoreNavEntryDecorator(viewModelStoreProvider: ViewModelStoreProvider): ViewModelStoreNavEntryDecorator<T>`
#[kompozable]
pub fn remember_view_model_store_nav_entry_decorator_of<T: NavEntryKey>(
    view_model_store_provider: ViewModelStoreProvider,
) -> NavEntryDecorator<T> {
    let p = view_model_store_provider.clone();
    remember_keyed(view_model_store_provider, move || ViewModelStoreNavEntryDecorator(p))
}

/// The same (Compose's deprecated `rememberViewModelStoreNavEntryDecorator(viewModelStoreOwner,
/// removeViewModelStoreOnPop)`: the lambda is ignored, never called).
///
/// Kotlin: `rememberViewModelStoreNavEntryDecorator(viewModelStoreOwner: ViewModelStoreOwner = checkNotNull(LocalViewModelStoreOwner.current) { "No ViewModelStoreOwner was provided via LocalViewModelStoreOwner" }, removeViewModelStoreOnPop: () -> Boolean = { true }): ViewModelStoreNavEntryDecorator<T>`
/// (Deprecated in Compose: use [`remember_view_model_store_nav_entry_decorator`].)
#[kompozable]
pub fn remember_view_model_store_nav_entry_decorator_removing<T: NavEntryKey>(
    #[default(required_view_model_store_owner())] view_model_store_owner: Rc<dyn ViewModelStoreOwner>,
    #[default(|| true)] remove_view_model_store_on_pop: impl Fn() -> bool + 'static,
) -> NavEntryDecorator<T> {
    let _ = remove_view_model_store_on_pop;
    remember_view_model_store_nav_entry_decorator(view_model_store_owner)
}

/// Compose's deprecated object `ViewModelStoreNavEntryDecoratorDefaults`.
#[deprecated(note = "This object is obsolete. To preserve decorator state outside of a NavDisplay, use the \
                     rememberDecoratedNavEntries API.")]
pub struct ViewModelStoreNavEntryDecoratorDefaults {
    _object: (),
}

#[allow(deprecated)]
impl ViewModelStoreNavEntryDecoratorDefaults {
    /// Whether a popped entry's store is cleared (always: `{ true }`).
    ///
    /// Kotlin: `removeViewModelStoreOnPop(): () -> Boolean`
    pub fn remove_view_model_store_on_pop() -> impl Fn() -> bool + Clone + 'static {
        || true
    }
}
