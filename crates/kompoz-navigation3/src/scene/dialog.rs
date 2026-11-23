//! DialogScene.kt: entries shown in a dialog.

use super::scene::{OverlayScene, Scene, SceneStrategy, SceneStrategyScope};
use crate::runtime::{Metadata, NavEntry, NavEntryKey, NavMetadataKey, metadata};
use kompoz_lifecycle::{LocalLifecycleOwner, remember_lifecycle_owner};
use kompoz_runtime::KompozitionLocalProvider;
use kompoz_ui::layout::SlotKey;
use kompoz_ui::window::{Dialog, DialogProperties};
use std::fmt;
use std::rc::Rc;

/// The scene of an entry in a [`Dialog`] (Compose's internal `DialogScene`), over the scene of the entries before.
pub(crate) struct DialogScene<T> {
    key: SlotKey,
    entry: NavEntry<T>,
    previous_entries: Vec<NavEntry<T>>,
    overlaid_entries: Vec<NavEntry<T>>,
    dialog_properties: DialogProperties,
    on_back: Rc<dyn Fn()>,
}

impl<T: PartialEq> PartialEq for DialogScene<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.previous_entries == other.previous_entries
            && self.overlaid_entries == other.overlaid_entries
            && self.entry == other.entry
            && self.dialog_properties == other.dialog_properties
    }
}

impl<T: fmt::Debug> fmt::Debug for DialogScene<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DialogScene(key={:?}, entry={:?}, previousEntries={:?}, overlaidEntries={:?}, dialogProperties={:?})",
            self.key, self.entry, self.previous_entries, self.overlaid_entries, self.dialog_properties
        )
    }
}

impl<T: NavEntryKey> Scene<T> for DialogScene<T> {
    fn key(&self) -> SlotKey {
        self.key.clone()
    }
    fn entries(&self) -> Vec<NavEntry<T>> {
        vec![self.entry.clone()]
    }
    fn previous_entries(&self) -> Vec<NavEntry<T>> {
        self.previous_entries.clone()
    }
    fn content(&self) {
        let lifecycle_owner = remember_lifecycle_owner();
        let on_back = self.on_back.clone();
        let entry = self.entry.clone();
        let properties = self.dialog_properties;
        kompoz_runtime::named_args!(Dialog(
            move || on_back(),
            properties = properties,
            content = move || {
                let entry = entry.clone();
                kompoz_runtime::kompozable_block!({
                    KompozitionLocalProvider(&[LocalLifecycleOwner.provides(lifecycle_owner.clone())], move || {
                        entry.Content()
                    });
                });
            }
        ));
    }
    fn as_overlay_scene(self: Rc<Self>) -> Option<Rc<dyn OverlayScene<T>>> {
        Some(self)
    }
}

impl<T: NavEntryKey> OverlayScene<T> for DialogScene<T> {
    fn overlaid_entries(&self) -> Vec<NavEntry<T>> {
        self.overlaid_entries.clone()
    }
}

/// The key of an entry's metadata showing it in a [`Dialog`] of these properties (Compose's
/// `DialogSceneStrategy.DialogKey`).
///
/// Kotlin: `DialogSceneStrategy.DialogKey`
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DialogKey;

impl NavMetadataKey<DialogProperties> for DialogKey {}

/// The strategy showing an entry whose metadata has a [`DialogKey`] in a [`Dialog`] (Compose's
/// `DialogSceneStrategy`), over the scene of the entries before it. Put before the strategies of other scenes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DialogSceneStrategy {
    _object: (),
}

/// Kotlin: `DialogSceneStrategy()`
pub fn DialogSceneStrategy() -> DialogSceneStrategy {
    DialogSceneStrategy { _object: () }
}

#[allow(non_upper_case_globals)]
impl DialogSceneStrategy {
    /// Kotlin: `DialogSceneStrategy.DialogKey`
    pub const DialogKey: DialogKey = DialogKey;

    /// The metadata of an entry shown in a dialog of `dialog_properties` (Compose's `DialogSceneStrategy.dialog(…)`).
    ///
    /// Kotlin: `dialog(dialogProperties: DialogProperties = DialogProperties()): Map<String, Any>`
    pub fn dialog(dialog_properties: DialogProperties) -> Metadata {
        metadata(|m| m.put(DialogKey, dialog_properties))
    }
}

impl<T: NavEntryKey> SceneStrategy<T> for DialogSceneStrategy {
    fn calculate_scene(&self, scope: &SceneStrategyScope<T>, entries: Vec<NavEntry<T>>) -> Option<Rc<dyn Scene<T>>> {
        let last = entries.last()?.clone();
        let properties = last.metadata().get(DialogKey)?;
        let before = entries[..entries.len() - 1].to_vec();
        Some(Rc::new(DialogScene {
            key: last.content_key(),
            entry: last,
            previous_entries: before.clone(),
            overlaid_entries: before,
            dialog_properties: properties,
            on_back: scope.on_back(),
        }))
    }
}
