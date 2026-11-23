//! An entry's owners in its content (navigation-compose's `NavBackStackEntry.LocalOwnersProvider`): the entry is
//! the content's ViewModel store owner, lifecycle owner and saved state registry owner, and its `rememberSaveable`
//! values are kept in the NavHost's saveable state holder under an id of its own, removed when the entry's
//! ViewModels are cleared (popped for good).

use crate::entry::{NavBackStackEntry, random_uuid};
use kompoz_lifecycle::{
    LocalLifecycleOwner, LocalSavedStateRegistryOwner, LocalViewModelStoreOwner, SavedStateHandle, ViewModel,
    ViewModelImpl, view_model_with,
};
use kompoz_runtime::{KompozitionLocalProvider, SaveableStateHolder, WeakSaveableStateHolder, group, kompozable_block};
use std::cell::RefCell;
use std::rc::Rc;

impl NavBackStackEntry {
    /// Composes `content` with this entry as its [`LocalViewModelStoreOwner`], [`LocalLifecycleOwner`] and
    /// [`LocalSavedStateRegistryOwner`], its saveable values kept by `saveable_state_holder` under the entry's own
    /// key (Compose's `NavBackStackEntry.LocalOwnersProvider`). Composable.
    ///
    /// Kotlin: `LocalOwnersProvider(saveableStateHolder: SaveableStateHolder, content: @Composable () -> Unit)`
    #[allow(non_snake_case)]
    #[track_caller]
    pub fn LocalOwnersProvider(&self, saveable_state_holder: &SaveableStateHolder, content: impl FnOnce()) {
        let content = RefCell::new(Some(content));
        let content = &content;
        let owners = [
            LocalViewModelStoreOwner.provides(self.as_view_model_store_owner()),
            LocalLifecycleOwner.provides(self.as_lifecycle_owner()),
            LocalSavedStateRegistryOwner.provides(self.as_saved_state_registry_owner()),
        ];
        let owners = &owners;
        kompozable_block!({
            KompozitionLocalProvider(owners, || {
                saveable_state_provider(saveable_state_holder, || {
                    KompozitionLocalProvider(owners, || {
                        if let Some(content) = content.borrow_mut().take() {
                            content();
                        }
                    });
                });
            });
        });
    }
}

/// `content` under the entry's key in `holder` (Compose's private `SaveableStateHolder.SaveableStateProvider`): the
/// key a ViewModel of the entry keeps (in its SavedStateHandle: the same after the process comes back), the holder's
/// state of it removed as the ViewModel is cleared.
#[track_caller]
fn saveable_state_provider(holder: &SaveableStateHolder, content: impl FnOnce()) {
    group(|| {
        let view_model: Rc<BackStackEntryIdViewModel> =
            view_model_with(|extras| BackStackEntryIdViewModel::new(extras.create_saved_state_handle()));
        *view_model.saveable_state_holder_ref.borrow_mut() = Some(holder.downgrade());
        holder.SaveableStateProvider(&view_model.id, content);
    });
}

/// The ViewModel keeping an entry's key in the NavHost's saveable state holder (Compose's internal
/// `BackStackEntryIdViewModel`).
pub(crate) struct BackStackEntryIdViewModel {
    base: ViewModelImpl,
    /// The key (a random UUID, kept in the handle).
    pub(crate) id: String,
    /// The holder (Kotlin's `WeakReference`).
    pub(crate) saveable_state_holder_ref: RefCell<Option<WeakSaveableStateHolder>>,
}

impl BackStackEntryIdViewModel {
    const ID_KEY: &'static str = "SaveableStateHolder_BackStackEntryKey";

    fn new(handle: SavedStateHandle) -> BackStackEntryIdViewModel {
        let id = match handle.get::<String>(Self::ID_KEY) {
            Some(id) => id,
            None => {
                let id = random_uuid();
                handle.set(Self::ID_KEY, id.clone());
                id
            }
        };
        BackStackEntryIdViewModel { base: ViewModelImpl::new(), id, saveable_state_holder_ref: RefCell::new(None) }
    }
}

impl ViewModel for BackStackEntryIdViewModel {
    fn view_model_impl(&self) -> &ViewModelImpl {
        &self.base
    }

    fn on_cleared(&self) {
        if let Some(holder) = self.saveable_state_holder_ref.borrow_mut().take().and_then(|w| w.upgrade()) {
            holder.remove_state(&self.id);
        }
    }
}
