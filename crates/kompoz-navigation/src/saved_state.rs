//! Navigation's arguments and saved states: savedstate's [`SavedState`] (kompoz-lifecycle's, a `Bundle` on Android),
//! read through a [`SavedStateReader`] and written through a [`SavedStateWriter`]; [`BundleGetters`] adds Android's
//! `Bundle` getters (Kotlin's `entry.arguments?.getInt("n")`).

pub use kompoz_lifecycle::{SavedState, SavedStateReader, SavedStateWriter, saved_state, saved_state_from};
use kompoz_runtime::SavedValue;

/// Android's `Bundle` getters on a [`SavedState`] (Kotlin's `entry.arguments?.getInt("n")`): a number or bool not
/// there (or not of the type) is 0 / `false`, as Android's; a string, an array, none.
pub trait BundleGetters {
    /// Whether it has `key` (Android's `containsKey`).
    fn contains_key(&self, key: &str) -> bool;
    /// Its keys (Android's `keySet`).
    fn key_set(&self) -> Vec<String>;
    fn get_int(&self, key: &str) -> i32;
    fn get_long(&self, key: &str) -> i64;
    fn get_float(&self, key: &str) -> f32;
    fn get_double(&self, key: &str) -> f64;
    fn get_boolean(&self, key: &str) -> bool;
    fn get_string(&self, key: &str) -> Option<String>;
    fn get_int_array(&self, key: &str) -> Option<Vec<i32>>;
    fn get_long_array(&self, key: &str) -> Option<Vec<i64>>;
    fn get_float_array(&self, key: &str) -> Option<Vec<f32>>;
    fn get_boolean_array(&self, key: &str) -> Option<Vec<bool>>;
    fn get_string_array(&self, key: &str) -> Option<Vec<String>>;
}

impl BundleGetters for SavedState {
    fn contains_key(&self, key: &str) -> bool {
        self.read(|r| r.contains(key))
    }
    fn key_set(&self) -> Vec<String> {
        self.read(|r| r.to_map().into_keys().collect())
    }
    fn get_int(&self, key: &str) -> i32 {
        self.read(|r| r.get_int_or_null(key)).unwrap_or(0)
    }
    fn get_long(&self, key: &str) -> i64 {
        self.read(|r| r.get_long_or_null(key)).unwrap_or(0)
    }
    fn get_float(&self, key: &str) -> f32 {
        self.read(|r| r.get_float_or_null(key)).unwrap_or(0.0)
    }
    fn get_double(&self, key: &str) -> f64 {
        self.read(|r| r.get_double_or_null(key)).unwrap_or(0.0)
    }
    fn get_boolean(&self, key: &str) -> bool {
        self.read(|r| r.get_boolean_or_null(key)).unwrap_or(false)
    }
    fn get_string(&self, key: &str) -> Option<String> {
        self.read(|r| r.get_string_or_null(key))
    }
    fn get_int_array(&self, key: &str) -> Option<Vec<i32>> {
        self.read(|r| r.get_int_array_or_null(key))
    }
    fn get_long_array(&self, key: &str) -> Option<Vec<i64>> {
        self.read(|r| r.get_long_array_or_null(key))
    }
    fn get_float_array(&self, key: &str) -> Option<Vec<f32>> {
        self.read(|r| r.get_float_array_or_null(key))
    }
    fn get_boolean_array(&self, key: &str) -> Option<Vec<bool>> {
        self.read(|r| r.get_boolean_array_or_null(key))
    }
    fn get_string_array(&self, key: &str) -> Option<Vec<String>> {
        self.read(|r| r.get_string_array_or_null(key))
    }
}

// ---- What navigation's code reads and writes with ----

pub(crate) fn contains(state: &SavedState, key: &str) -> bool {
    state.read(|r| r.contains(key))
}

pub(crate) fn is_null(state: &SavedState, key: &str) -> bool {
    state.read(|r| r.is_null(key))
}

pub(crate) fn put(state: &SavedState, key: &str, value: SavedValue) {
    state.write(|w| w.put_value(key, value));
}

pub(crate) fn put_all(into: &SavedState, from: &SavedState) {
    into.write(|w| w.put_all(from));
}

pub(crate) fn size(state: &SavedState) -> usize {
    state.read(|r| r.size()) as usize
}

pub(crate) fn keys(state: &SavedState) -> Vec<String> {
    state.read(|r| r.to_map().into_keys().collect())
}

/// A copy of `state` (Kotlin's `savedState { putAll(state) }`).
pub(crate) fn copy(state: &SavedState) -> SavedState {
    saved_state_from(state, |_| {})
}
