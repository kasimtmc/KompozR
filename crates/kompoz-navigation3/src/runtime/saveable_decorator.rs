//! SaveableStateHolderNavEntryDecorator.kt.

use super::nav_entry::{NavEntry, NavEntryDecorator};
use super::nav_key::NavEntryKey;
use kompoz_runtime::kt::panic_kt;
use kompoz_runtime::{SaveableStateHolder, kompozable, remember_keyed, remember_saveable_state_holder};
use kompoz_ui::layout::SlotKey;

/// A content key as a [`SaveableStateHolder`]'s key (Compose's `SaveableStateProvider(key: Any)`: a key must be
/// saveable — a text or a number —, else "Type of the key … is not supported"). A text is its own key; a number or
/// another saveable value its kind's name before it (so that `1` and `"1"` are two keys, as Kotlin's `Any` keys are).
pub(crate) fn saveable_key(key: &SlotKey) -> String {
    if let Some(s) = key.downcast_ref::<String>() {
        return s.clone();
    }
    if let Some(s) = key.downcast_ref::<&'static str>() {
        return s.to_string();
    }
    macro_rules! kinds {
        ($($t:ty => $name:literal),*) => {$(
            if let Some(v) = key.downcast_ref::<$t>() {
                return format!("{}:{v}", $name);
            }
        )*};
    }
    kinds!(i32 => "Int", i64 => "Long", i16 => "Short", i8 => "Byte", u32 => "UInt", u64 => "ULong", usize => "Long",
        char => "Char", bool => "Boolean");
    panic_kt(
        "IllegalArgumentException",
        &format!(
            "Type of the key {key:?} is not supported. On Android you can only use types which can be stored inside \
             the Bundle."
        ),
    )
}

/// What [`SaveableStateHolderNavEntryDecorator`] makes (Compose's class of that name, a `NavEntryDecorator`).
pub type SaveableStateHolderNavEntryDecorator<T> = NavEntryDecorator<T>;

/// The decorator keeping each entry's `remember_saveable` state in `saveable_state_holder` under its content key
/// (Compose's `SaveableStateHolderNavEntryDecorator`: its content in `SaveableStateProvider(contentKey)`), the state
/// of a popped content key removed. The one decorator that is required: saving state isn't optional.
///
/// Kotlin: `SaveableStateHolderNavEntryDecorator(saveableStateHolder: SaveableStateHolder)`
pub fn SaveableStateHolderNavEntryDecorator<T: NavEntryKey>(
    saveable_state_holder: SaveableStateHolder,
) -> NavEntryDecorator<T> {
    let (popped, holder) = (saveable_state_holder.clone(), saveable_state_holder);
    NavEntryDecorator::new(
        move |content_key: SlotKey| popped.remove_state(&saveable_key(&content_key)),
        move |entry: NavEntry<T>| {
            let key = saveable_key(&entry.content_key());
            holder.SaveableStateProvider(&key, || entry.Content());
        },
    )
}

/// A [`SaveableStateHolderNavEntryDecorator`] of `saveable_state_holder` remembered across recompositions (Compose's
/// `rememberSaveableStateHolderNavEntryDecorator`): by default of a holder remembered and saved here.
///
/// Kotlin: `rememberSaveableStateHolderNavEntryDecorator(saveableStateHolder: SaveableStateHolder = rememberSaveableStateHolder()): SaveableStateHolderNavEntryDecorator<T>`
#[kompozable]
pub fn remember_saveable_state_holder_nav_entry_decorator<T: NavEntryKey>(
    #[default(remember_saveable_state_holder())] saveable_state_holder: SaveableStateHolder,
) -> NavEntryDecorator<T> {
    let holder = saveable_state_holder.clone();
    remember_keyed(saveable_state_holder, move || SaveableStateHolderNavEntryDecorator(holder))
}
