//! androidx.savedstate 1.4 (its desktop artifact; savedstate-compose's `LocalSavedStateRegistryOwner`): what is
//! written out while the platform ends the app and given back when it is made again —
//!
//! - [`SavedState`], a map of keys to [`SavedValue`]s (Kotlin's `SavedState`, a map of keys to values a Bundle can
//!   hold), built with [`saved_state`] and read and written through a [`SavedStateReader`] / [`SavedStateWriter`]
//!   (`read { }`, `write { }`);
//! - [`SavedStateRegistry`] (the components of an owner registering [`SavedStateProvider`]s by key and consuming
//!   what they saved), [`SavedStateRegistryOwner`], [`SavedStateRegistryController`] (what the owner restores and
//!   saves it with), [`LocalSavedStateRegistryOwner`];
//! - savedstate's serialization (kotlinx.serialization's `encodeToSavedState`, `decodeFromSavedState`, the `saved { }`
//!   delegates) over KompozR's [`Saveable`] (`#[derive(Saveable)]`): [`encode_to_saved_state`],
//!   [`decode_from_saved_state`], [`SavedStateRegistryOwnerExtensions::saved`].

mod compose;
mod configuration;
mod registry;
mod serialization;

pub use compose::LocalSavedStateRegistryOwner;
pub use configuration::{
    PolymorphicBase, PolymorphicModuleBuilder, SavedStateConfiguration, SavedStateConfigurationBuilder, SerialClass,
    SerializersModule, SerializersModuleBuilder, configured_saver, polymorphic_restorer, remember_serializable,
    remember_serializable_configured,
};
pub use registry::{SavedStateProvider, SavedStateRegistry, SavedStateRegistryController, SavedStateRegistryOwner};
pub use serialization::{
    SavedStateDelegate, SavedStateRegistryOwnerExtensions, decode_from_saved_state, decode_from_saved_state_configured,
    decode_from_saved_state_with, encode_to_saved_state, encode_to_saved_state_configured, encode_to_saved_state_with,
};

use kompoz_runtime::{Saveable, SavedValue};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

/// Values by key, saved and restored as the platform ends the app and brings it back (Compose's `SavedState`, a
/// Bundle on Android): each a [`SavedValue`] — a bool, a number, a text, a list, or a saved state of its own (a
/// map) —; read with [`read`](Self::read), written with [`write`](Self::write). A clone is the same object (Kotlin's
/// reference). Two compare equal when their contents do (Kotlin's `contentDeepEquals`; Kotlin's `==` is identity).
///
/// A saved state put into another ([`SavedStateWriter::put_saved_state`]) is copied there: writing to it afterwards
/// doesn't change the other (a Kotlin Bundle holds the same object).
#[derive(Clone, Default)]
pub struct SavedState(Rc<RefCell<BTreeMap<String, SavedValue>>>);

impl SavedState {
    /// An empty one (Kotlin's `savedState()`).
    pub fn new() -> SavedState {
        SavedState::default()
    }

    /// One of the entries of a saved value that is a map (a saved state written out); none of anything else.
    pub fn from_saved_value(value: &SavedValue) -> SavedState {
        let map = value.as_map().cloned().unwrap_or_default();
        SavedState(Rc::new(RefCell::new(map)))
    }

    /// Its entries as a saved value (a map): what it is written out as.
    pub fn to_saved_value(&self) -> SavedValue {
        SavedValue::Map(Arc::new(self.0.borrow().clone()))
    }

    /// Calls `block` with a [`SavedStateReader`] of it (Kotlin's `SavedState.read { … }`), giving what it gives.
    pub fn read<T>(&self, block: impl FnOnce(&SavedStateReader) -> T) -> T {
        block(&SavedStateReader { source: self.clone() })
    }

    /// Calls `block` with a [`SavedStateWriter`] of it (Kotlin's `SavedState.write { … }`), giving what it gives.
    pub fn write<T>(&self, block: impl FnOnce(&SavedStateWriter) -> T) -> T {
        block(&SavedStateWriter { source: self.clone() })
    }

    /// Whether `self` and `other` are the same object (Kotlin's `===`).
    pub fn ptr_eq(&self, other: &SavedState) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    fn get(&self, key: &str) -> Option<SavedValue> {
        self.0.borrow().get(key).cloned()
    }

    fn put(&self, key: &str, value: SavedValue) {
        self.0.borrow_mut().insert(key.to_string(), value);
    }
}

impl PartialEq for SavedState {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other) || *self.0.borrow() == *other.0.borrow()
    }
}

impl fmt::Debug for SavedState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.read(|r| r.content_deep_to_string()))
    }
}

/// A saved state is saved as itself (Kotlin's SavedState in a Bundle, a `SavedStateHandle`'s value).
impl Saveable for SavedState {
    fn save(&self) -> SavedValue {
        self.to_saved_value()
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        saved.as_map().map(|_| SavedState::from_saved_value(saved))
    }
}

/// A new saved state, `builder_action` writing it (Compose's `savedState(builderAction)`; Kotlin's
/// `savedState()` is [`SavedState::new`]).
///
/// ```
/// # use kompoz_lifecycle::*;
/// let user = saved_state(|s| {
///     s.put_int("id", 123);
///     s.put_string("name", "foo");
/// });
/// assert_eq!(user.read(|r| r.get_string("name")), "foo");
/// ```
///
/// Kotlin: `savedState(builderAction: SavedStateWriter.() -> Unit = {})`
pub fn saved_state(builder_action: impl FnOnce(&SavedStateWriter)) -> SavedState {
    let state = SavedState::new();
    state.write(builder_action);
    state
}

/// A new saved state of `initial_state`'s entries (copied), `builder_action` writing it (Compose's
/// `savedState(initialState: Map<String, Any?>, builderAction)`).
///
/// Kotlin: `savedState(initialState: Map<String, Any?>, builderAction: SavedStateWriter.() -> Unit = {})`
pub fn saved_state_of<K: Into<String>>(
    initial_state: impl IntoIterator<Item = (K, SavedValue)>,
    builder_action: impl FnOnce(&SavedStateWriter),
) -> SavedState {
    let state = SavedState(Rc::new(RefCell::new(initial_state.into_iter().map(|(k, v)| (k.into(), v)).collect())));
    state.write(builder_action);
    state
}

/// A new saved state of `initial_state`'s entries (a copy), `builder_action` writing it (Compose's
/// `savedState(initialState: SavedState, builderAction)`).
///
/// Kotlin: `savedState(initialState: SavedState, builderAction: SavedStateWriter.() -> Unit = {})`
pub fn saved_state_from(initial_state: &SavedState, builder_action: impl FnOnce(&SavedStateWriter)) -> SavedState {
    let state = SavedState(Rc::new(RefCell::new(initial_state.0.borrow().clone())));
    state.write(builder_action);
    state
}

/// Compose's `keyOrValueNotFoundError` (an IllegalArgumentException).
#[track_caller]
fn key_or_value_not_found(key: &str) -> ! {
    panic!(
        "IllegalArgumentException: No valid saved state was found for the key '{key}'. It may be missing, null, or \
         not of the expected type. This can occur if the value was saved with a different type or if the saved \
         state was modified unexpectedly."
    )
}

fn as_i32(v: &SavedValue) -> Option<i32> {
    v.as_int().and_then(|n| i32::try_from(n).ok())
}

fn as_char(v: &SavedValue) -> Option<char> {
    let s = v.as_str()?;
    let mut chars = s.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

fn list_of<T>(v: &SavedValue, each: impl Fn(&SavedValue) -> Option<T>) -> Option<Vec<T>> {
    v.as_list()?.iter().map(each).collect()
}

/// Reads a [`SavedState`] (Compose's `SavedStateReader`, the receiver of `read { }`): a value of a key as a type,
/// panicking when there is none of that type (`getInt`), or none then (`getIntOrNull`). Numbers are kept as Kotlin's
/// widest (an `Int` read as a `Long` too, a `Float` as a `Double`); a `Char` is a one-letter string.
pub struct SavedStateReader {
    source: SavedState,
}

impl SavedStateReader {
    /// A bool.
    #[track_caller]
    pub fn get_boolean(&self, key: &str) -> bool {
        self.get_boolean_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_boolean_or_null(&self, key: &str) -> Option<bool> {
        let conv: fn(&SavedValue) -> Option<bool> = SavedValue::as_bool;
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A character.
    #[track_caller]
    pub fn get_char(&self, key: &str) -> char {
        self.get_char_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_char_or_null(&self, key: &str) -> Option<char> {
        let conv: fn(&SavedValue) -> Option<char> = as_char;
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A text (Kotlin's `CharSequence`).
    #[track_caller]
    pub fn get_char_sequence(&self, key: &str) -> String {
        self.get_char_sequence_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_char_sequence_or_null(&self, key: &str) -> Option<String> {
        let conv: fn(&SavedValue) -> Option<String> = |v| v.as_str().map(str::to_string);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A number.
    #[track_caller]
    pub fn get_double(&self, key: &str) -> f64 {
        self.get_double_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_double_or_null(&self, key: &str) -> Option<f64> {
        let conv: fn(&SavedValue) -> Option<f64> = SavedValue::as_f64;
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A number, as a `Float`.
    #[track_caller]
    pub fn get_float(&self, key: &str) -> f32 {
        self.get_float_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_float_or_null(&self, key: &str) -> Option<f32> {
        let conv: fn(&SavedValue) -> Option<f32> = |v| v.as_f64().map(|d| d as f32);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A whole number that fits an `Int`.
    #[track_caller]
    pub fn get_int(&self, key: &str) -> i32 {
        self.get_int_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_int_or_null(&self, key: &str) -> Option<i32> {
        let conv: fn(&SavedValue) -> Option<i32> = as_i32;
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A whole number.
    #[track_caller]
    pub fn get_long(&self, key: &str) -> i64 {
        self.get_long_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_long_or_null(&self, key: &str) -> Option<i64> {
        let conv: fn(&SavedValue) -> Option<i64> = SavedValue::as_int;
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A text.
    #[track_caller]
    pub fn get_string(&self, key: &str) -> String {
        self.get_string_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_string_or_null(&self, key: &str) -> Option<String> {
        let conv: fn(&SavedValue) -> Option<String> = |v| v.as_str().map(str::to_string);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A list of texts.
    #[track_caller]
    pub fn get_char_sequence_list(&self, key: &str) -> Vec<String> {
        self.get_char_sequence_list_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_char_sequence_list_or_null(&self, key: &str) -> Option<Vec<String>> {
        let conv: fn(&SavedValue) -> Option<Vec<String>> = |v| list_of(v, |e| e.as_str().map(str::to_string));
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A list of `Int`s.
    #[track_caller]
    pub fn get_int_list(&self, key: &str) -> Vec<i32> {
        self.get_int_list_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_int_list_or_null(&self, key: &str) -> Option<Vec<i32>> {
        let conv: fn(&SavedValue) -> Option<Vec<i32>> = |v| list_of(v, as_i32);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A list of saved states.
    #[track_caller]
    pub fn get_saved_state_list(&self, key: &str) -> Vec<SavedState> {
        self.get_saved_state_list_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_saved_state_list_or_null(&self, key: &str) -> Option<Vec<SavedState>> {
        let conv: fn(&SavedValue) -> Option<Vec<SavedState>> = |v| list_of(v, SavedState::restore);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A list of texts.
    #[track_caller]
    pub fn get_string_list(&self, key: &str) -> Vec<String> {
        self.get_string_list_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_string_list_or_null(&self, key: &str) -> Option<Vec<String>> {
        let conv: fn(&SavedValue) -> Option<Vec<String>> = |v| list_of(v, |e| e.as_str().map(str::to_string));
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of characters.
    #[track_caller]
    pub fn get_char_array(&self, key: &str) -> Vec<char> {
        self.get_char_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_char_array_or_null(&self, key: &str) -> Option<Vec<char>> {
        let conv: fn(&SavedValue) -> Option<Vec<char>> = |v| list_of(v, as_char);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of texts.
    #[track_caller]
    pub fn get_char_sequence_array(&self, key: &str) -> Vec<String> {
        self.get_char_sequence_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_char_sequence_array_or_null(&self, key: &str) -> Option<Vec<String>> {
        let conv: fn(&SavedValue) -> Option<Vec<String>> = |v| list_of(v, |e| e.as_str().map(str::to_string));
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of bools.
    #[track_caller]
    pub fn get_boolean_array(&self, key: &str) -> Vec<bool> {
        self.get_boolean_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_boolean_array_or_null(&self, key: &str) -> Option<Vec<bool>> {
        let conv: fn(&SavedValue) -> Option<Vec<bool>> = |v| list_of(v, SavedValue::as_bool);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of numbers.
    #[track_caller]
    pub fn get_double_array(&self, key: &str) -> Vec<f64> {
        self.get_double_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_double_array_or_null(&self, key: &str) -> Option<Vec<f64>> {
        let conv: fn(&SavedValue) -> Option<Vec<f64>> = |v| list_of(v, SavedValue::as_f64);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of `Float`s.
    #[track_caller]
    pub fn get_float_array(&self, key: &str) -> Vec<f32> {
        self.get_float_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_float_array_or_null(&self, key: &str) -> Option<Vec<f32>> {
        let conv: fn(&SavedValue) -> Option<Vec<f32>> = |v| list_of(v, |e| e.as_f64().map(|d| d as f32));
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of `Int`s.
    #[track_caller]
    pub fn get_int_array(&self, key: &str) -> Vec<i32> {
        self.get_int_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_int_array_or_null(&self, key: &str) -> Option<Vec<i32>> {
        let conv: fn(&SavedValue) -> Option<Vec<i32>> = |v| list_of(v, as_i32);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of whole numbers.
    #[track_caller]
    pub fn get_long_array(&self, key: &str) -> Vec<i64> {
        self.get_long_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_long_array_or_null(&self, key: &str) -> Option<Vec<i64>> {
        let conv: fn(&SavedValue) -> Option<Vec<i64>> = |v| list_of(v, SavedValue::as_int);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of saved states.
    #[track_caller]
    pub fn get_saved_state_array(&self, key: &str) -> Vec<SavedState> {
        self.get_saved_state_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_saved_state_array_or_null(&self, key: &str) -> Option<Vec<SavedState>> {
        let conv: fn(&SavedValue) -> Option<Vec<SavedState>> = |v| list_of(v, SavedState::restore);
        self.source.get(key).as_ref().and_then(conv)
    }

    /// An array of texts.
    #[track_caller]
    pub fn get_string_array(&self, key: &str) -> Vec<String> {
        self.get_string_array_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_string_array_or_null(&self, key: &str) -> Option<Vec<String>> {
        let conv: fn(&SavedValue) -> Option<Vec<String>> = |v| list_of(v, |e| e.as_str().map(str::to_string));
        self.source.get(key).as_ref().and_then(conv)
    }

    /// A saved state put in it (a copy).
    #[track_caller]
    pub fn get_saved_state(&self, key: &str) -> SavedState {
        self.get_saved_state_or_null(key).unwrap_or_else(|| key_or_value_not_found(key))
    }

    /// The same, none if there is no such value.
    pub fn get_saved_state_or_null(&self, key: &str) -> Option<SavedState> {
        let conv: fn(&SavedValue) -> Option<SavedState> = SavedState::restore;
        self.source.get(key).as_ref().and_then(conv)
    }

    /// Its value of `key` as it is held (none if there is none): Rust's way to Kotlin's untyped `toMap()[key]`.
    pub fn get_value(&self, key: &str) -> Option<SavedValue> {
        self.source.get(key)
    }

    /// How many values it has.
    pub fn size(&self) -> i32 {
        self.source.0.borrow().len() as i32
    }

    pub fn is_empty(&self) -> bool {
        self.source.0.borrow().is_empty()
    }

    /// Whether it has `key`, its value null.
    pub fn is_null(&self, key: &str) -> bool {
        matches!(self.source.get(key), Some(SavedValue::Null))
    }

    /// Whether it has `key` (Kotlin's `key in reader`).
    pub fn contains(&self, key: &str) -> bool {
        self.source.0.borrow().contains_key(key)
    }

    /// Whether `other` has the same contents, all the way down.
    pub fn content_deep_equals(&self, other: &SavedState) -> bool {
        self.source == *other
    }

    /// A hash of its contents, all the way down (equal for states `content_deep_equals` says are).
    pub fn content_deep_hash_code(&self) -> i32 {
        fn hash(v: &SavedValue, h: &mut std::collections::hash_map::DefaultHasher) {
            match v {
                SavedValue::Null => 0u8.hash(h),
                SavedValue::Bool(b) => b.hash(h),
                SavedValue::Int(n) => n.hash(h),
                SavedValue::Double(d) => d.to_bits().hash(h),
                SavedValue::String(s) => s.hash(h),
                SavedValue::List(l) => l.iter().for_each(|e| hash(e, h)),
                SavedValue::Map(m) => m.iter().for_each(|(k, e)| {
                    k.hash(h);
                    hash(e, h)
                }),
            }
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        hash(&self.source.to_saved_value(), &mut h);
        h.finish() as i32
    }

    /// Its contents written out (Kotlin's `contentDeepToString`: `[id=123, name=foo]`, nested states and lists in
    /// brackets).
    pub fn content_deep_to_string(&self) -> String {
        fn write(v: &SavedValue, out: &mut String) {
            match v {
                SavedValue::Null => out.push_str("null"),
                SavedValue::Bool(b) => out.push_str(&b.to_string()),
                SavedValue::Int(n) => out.push_str(&n.to_string()),
                SavedValue::Double(d) => out.push_str(&kompoz_runtime::kt::KtToString::kt_to_string(d)),
                SavedValue::String(s) => out.push_str(s),
                SavedValue::List(l) => {
                    out.push('[');
                    for (i, e) in l.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        write(e, out);
                    }
                    out.push(']');
                }
                SavedValue::Map(m) => {
                    out.push('[');
                    for (i, (k, e)) in m.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        out.push_str(k);
                        out.push('=');
                        write(e, out);
                    }
                    out.push(']');
                }
            }
        }
        let mut out = String::new();
        write(&self.source.to_saved_value(), &mut out);
        out
    }

    /// Its values by key (a copy).
    pub fn to_map(&self) -> BTreeMap<String, SavedValue> {
        self.source.0.borrow().clone()
    }
}

/// Writes a [`SavedState`] (Compose's `SavedStateWriter`, the receiver of `savedState { }` and `write { }`): a
/// value put under a key replaces the one there.
pub struct SavedStateWriter {
    source: SavedState,
}

impl SavedStateWriter {
    pub fn put_boolean(&self, key: &str, value: bool) {
        self.source.put(key, SavedValue::Bool(value));
    }
    pub fn put_char(&self, key: &str, value: char) {
        self.source.put(key, SavedValue::String(value.to_string()));
    }
    pub fn put_char_sequence(&self, key: &str, value: impl Into<String>) {
        self.source.put(key, SavedValue::String(value.into()));
    }
    pub fn put_double(&self, key: &str, value: f64) {
        self.source.put(key, SavedValue::Double(value));
    }
    pub fn put_float(&self, key: &str, value: f32) {
        self.source.put(key, SavedValue::Double(value as f64));
    }
    pub fn put_int(&self, key: &str, value: i32) {
        self.source.put(key, SavedValue::Int(value as i64));
    }
    pub fn put_long(&self, key: &str, value: i64) {
        self.source.put(key, SavedValue::Int(value));
    }
    /// A null value under `key` (there, and null).
    pub fn put_null(&self, key: &str) {
        self.source.put(key, SavedValue::Null);
    }
    pub fn put_string(&self, key: &str, value: impl Into<String>) {
        self.source.put(key, SavedValue::String(value.into()));
    }
    pub fn put_char_sequence_list(&self, key: &str, value: impl IntoIterator<Item = impl Into<String>>) {
        self.source.put(key, SavedValue::list(value.into_iter().map(|s| SavedValue::String(s.into()))));
    }
    pub fn put_int_list(&self, key: &str, value: impl IntoIterator<Item = i32>) {
        self.source.put(key, SavedValue::list(value.into_iter().map(SavedValue::from)));
    }
    pub fn put_saved_state_list(&self, key: &str, value: impl IntoIterator<Item = SavedState>) {
        self.source.put(key, SavedValue::list(value.into_iter().map(|s| s.to_saved_value())));
    }
    pub fn put_string_list(&self, key: &str, value: impl IntoIterator<Item = impl Into<String>>) {
        self.put_char_sequence_list(key, value);
    }
    pub fn put_boolean_array(&self, key: &str, value: impl IntoIterator<Item = bool>) {
        self.source.put(key, SavedValue::list(value.into_iter().map(SavedValue::Bool)));
    }
    pub fn put_char_array(&self, key: &str, value: impl IntoIterator<Item = char>) {
        self.source.put(key, SavedValue::list(value.into_iter().map(|c| SavedValue::String(c.to_string()))));
    }
    pub fn put_char_sequence_array(&self, key: &str, value: impl IntoIterator<Item = impl Into<String>>) {
        self.put_char_sequence_list(key, value);
    }
    pub fn put_double_array(&self, key: &str, value: impl IntoIterator<Item = f64>) {
        self.source.put(key, SavedValue::list(value.into_iter().map(SavedValue::Double)));
    }
    pub fn put_float_array(&self, key: &str, value: impl IntoIterator<Item = f32>) {
        self.source.put(key, SavedValue::list(value.into_iter().map(SavedValue::from)));
    }
    pub fn put_int_array(&self, key: &str, value: impl IntoIterator<Item = i32>) {
        self.put_int_list(key, value);
    }
    pub fn put_long_array(&self, key: &str, value: impl IntoIterator<Item = i64>) {
        self.source.put(key, SavedValue::list(value.into_iter().map(SavedValue::Int)));
    }
    pub fn put_saved_state_array(&self, key: &str, value: impl IntoIterator<Item = SavedState>) {
        self.put_saved_state_list(key, value);
    }
    pub fn put_string_array(&self, key: &str, value: impl IntoIterator<Item = impl Into<String>>) {
        self.put_char_sequence_list(key, value);
    }
    /// `value` under `key` (its contents as they are now: a copy).
    pub fn put_saved_state(&self, key: &str, value: &SavedState) {
        self.source.put(key, value.to_saved_value());
    }
    /// A value as it is held (Rust's way to put what a [`SavedStateReader::get_value`] gave, or any [`Saveable`]'s
    /// saved form).
    pub fn put_value(&self, key: &str, value: SavedValue) {
        self.source.put(key, value);
    }
    /// All of `from`'s values (replacing those of the same keys).
    pub fn put_all(&self, from: &SavedState) {
        if self.source.ptr_eq(from) {
            return;
        }
        let entries = from.0.borrow().clone();
        self.source.0.borrow_mut().extend(entries);
    }
    pub fn remove(&self, key: &str) {
        self.source.0.borrow_mut().remove(key);
    }
    pub fn clear(&self) {
        self.source.0.borrow_mut().clear();
    }
}
