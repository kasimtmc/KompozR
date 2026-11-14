//! KompozR's composition runtime, as Jetpack Compose's: composable functions that make a tree of groups, states
//! kept in snapshots, smart recomposition (only the scopes that read a changed state run again; unchanged ones
//! are skipped), `remember`, effects, composition locals, movable content and saveable state.
//!
//! It knows nothing of layout or drawing: what composables emit is a [`Node`] the UI layer defines, and a host
//! drives a [`Composition`] frame by frame.

#![allow(non_upper_case_globals)]

extern crate self as kompoz_runtime;

#[doc(hidden)]
pub mod __private {
    pub use crate::private::*;
}
mod collections;
mod composer;
mod derived;
mod local;
mod movable;
mod private;
mod read;
mod remember;
mod saveable;
mod saved_value;
mod snapshot;
mod state;

pub use collections::{SnapshotStateList, SnapshotStateMap, mutable_state_list_of, mutable_state_map_of};
pub use composer::{Composition, Content, Node, RecompositionStats, RememberObserver};
pub use derived::{DerivedState, derived_state_of, derived_state_of_with_policy};
pub use kompoz_macros::{kompozable, kompozable_block, kompozable_lambda, named};
pub use local::{
    KompozitionLocal, KompozitionLocalContext, KompozitionLocalProvider, KompozitionLocalProviderContext,
    ProvidedValue, current_kompozition_local_context, current_kompozition_local_context_unobserved,
    kompozition_local_of, static_kompozition_local_of,
};
pub use movable::{MovableContent, MovableContentOf, movable_content_of, movable_content_with_param_of};
pub use read::{observe_reads, record_read, without_read_observation};
pub use remember::{
    DisposableEffect, DisposableEffectResult, SideEffect, UpdatedState, current_composite_key_hash, emit,
    frame_time_nanos, group, in_composition, key, on_dispose, remember, remember_keyed, remember_observer,
    remember_observer_keyed, remember_updated_state, request_frame, request_frame_at, restartable, scoped,
};
pub use saveable::{
    ProvideSaveableStateRegistry, SaveableStateHolder, SaveableStateRegistry, Saver, list_saver, map_saver,
    remember_saveable, remember_saveable_keyed, remember_saveable_keyed_with, remember_saveable_state_holder,
    remember_saveable_with,
};
pub use saved_value::{Saveable, SavedValue};
pub use snapshot::{
    ApplyObserver, MutableSnapshot, ObserverHandle, ReadObserver, Snapshot, SnapshotApplyConflict, SnapshotApplyResult,
    StateKey, WriteObserver, global_snapshot_changed, set_global_write_observer, set_ui_thread, state_changed,
};
pub use state::{
    MutableState, SnapshotMutationPolicy, State, StateSource, mutable_double_state_of, mutable_float_state_of,
    mutable_int_state_of, mutable_long_state_of, mutable_state_of, mutable_state_of_with_policy, never_equal_policy,
    structural_equality_policy,
};
