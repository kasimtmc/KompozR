//! Observable value holders: `MutableState`, the read-only `State`, mutation policies.

use crate::read::record_read;
use crate::snapshot::{StateCell, StateKey};
use std::fmt;
use std::sync::Arc;

/// When a new value counts as the one a state has (Compose's SnapshotMutationPolicy): setting a state to a value
/// equivalent to its own changes nothing and recomposes no one. Without `equivalent`, never equivalent.
pub struct SnapshotMutationPolicy<T> {
    pub(crate) equivalent: Option<fn(&T, &T) -> bool>,
}

impl<T> Clone for SnapshotMutationPolicy<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for SnapshotMutationPolicy<T> {}

impl<T> SnapshotMutationPolicy<T> {
    /// A policy of its own: `equivalent(a, b)` says whether `b` changes nothing where `a` is.
    pub const fn new(equivalent: fn(&T, &T) -> bool) -> Self {
        SnapshotMutationPolicy { equivalent: Some(equivalent) }
    }
}

/// Equal by `==`: the default.
pub fn structural_equality_policy<T: PartialEq>() -> SnapshotMutationPolicy<T> {
    SnapshotMutationPolicy::new(|a, b| a == b)
}

/// Never equivalent: every `set` recomposes the readers, the same value too.
pub fn never_equal_policy<T>() -> SnapshotMutationPolicy<T> {
    SnapshotMutationPolicy { equivalent: None }
}

/// An observable value. Copies (clones) share the same value, so a state can be captured by value in event
/// callbacks. Reading it in a composable subscribes that composable; setting it recomposes exactly the readers.
/// Its value is kept per snapshot: see [`Snapshot`](crate::Snapshot).
pub struct MutableState<T> {
    cell: Arc<StateCell<T>>,
}

impl<T> Clone for MutableState<T> {
    fn clone(&self) -> Self {
        MutableState { cell: self.cell.clone() }
    }
}

impl<T: Clone + Send + Sync + 'static> MutableState<T> {
    pub fn new(value: T, policy: SnapshotMutationPolicy<T>) -> Self {
        MutableState { cell: StateCell::new(value, policy.equivalent) }
    }

    /// The value the snapshot this runs in sees (a copy).
    pub fn get(&self) -> T {
        self.with(T::clone)
    }

    /// Runs `f` with the value, without copying it.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        record_read(self.cell.key());
        self.cell.with(true, f)
    }

    /// The value, without subscribing the running composable.
    pub fn peek(&self) -> T {
        self.cell.with(false, T::clone)
    }

    pub fn peek_with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.cell.with(false, f)
    }

    /// Nothing happens if `value` is equivalent to the one it has (its policy; by default `==`).
    pub fn set(&self, value: T) {
        self.cell.set(value);
    }

    /// Its value (Compose's `state.value`, a read that subscribes): the same as [`get`](MutableState::get).
    pub fn value(&self) -> T {
        self.get()
    }

    /// `state.value = v` (Compose's): the same as [`set`](MutableState::set).
    pub fn set_value(&self, value: T) {
        self.set(value);
    }

    /// `val (value, setValue) = state`'s first part (Compose's `component1`): its value.
    pub fn component1(&self) -> T {
        self.get()
    }

    /// `val (value, setValue) = state`'s second part (Compose's `component2`): what sets it.
    pub fn component2(&self) -> impl Fn(T) + Clone + 'static {
        let state = self.clone();
        move |value| state.set(value)
    }

    /// Changes the value in place: `todos.update(|list| list.push(todo))`.
    pub fn update(&self, mutate: impl FnOnce(&mut T)) {
        self.cell.update(mutate);
    }

    /// Who its readers are recorded under.
    pub fn key(&self) -> StateKey {
        self.cell.key()
    }

    /// How many records it keeps (what the snapshot tests count).
    #[doc(hidden)]
    pub fn record_count(&self) -> usize {
        self.cell.records()
    }
}

/// The same state, not the same value: lets composables taking a state be skipped.
impl<T> PartialEq for MutableState<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cell, &other.cell)
    }
}
impl<T> Eq for MutableState<T> {}

/// Formats the value (reading it): `format!("Count: {count}")`.
impl<T: fmt::Display + Clone + Send + Sync + 'static> fmt::Display for MutableState<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.with(|v| v.fmt(f))
    }
}

impl<T: fmt::Debug + Clone + Send + Sync + 'static> fmt::Debug for MutableState<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.peek_with(|v| f.debug_tuple("MutableState").field(v).finish())
    }
}

/// `mutableStateOf(value)`: a state whose new values count as the old one when `==`.
pub fn mutable_state_of<T: PartialEq + Clone + Send + Sync + 'static>(value: T) -> MutableState<T> {
    MutableState::new(value, structural_equality_policy())
}

/// `mutableStateOf(value, policy)`.
///
/// Kotlin: `mutableStateOf(value: T, policy: SnapshotMutationPolicy<T>)`
pub fn mutable_state_of_with_policy<T: Clone + Send + Sync + 'static>(
    value: T,
    policy: SnapshotMutationPolicy<T>,
) -> MutableState<T> {
    MutableState::new(value, policy)
}

/// Compose's primitive states; here plain `mutable_state_of` already keeps the value unboxed.
pub fn mutable_int_state_of(value: i32) -> MutableState<i32> {
    mutable_state_of(value)
}
pub fn mutable_long_state_of(value: i64) -> MutableState<i64> {
    mutable_state_of(value)
}
pub fn mutable_float_state_of(value: f32) -> MutableState<f32> {
    mutable_state_of(value)
}
pub fn mutable_double_state_of(value: f64) -> MutableState<f64> {
    mutable_state_of(value)
}

impl<T: Clone + Send + Sync + 'static> MutableState<T> {
    /// A read-only view of it, `Send + Sync` as it is (see [`SharedState`]).
    pub fn as_shared_state(&self) -> SharedState<T> {
        SharedState { state: self.clone() }
    }
}

/// A read-only view of a [`MutableState`], `Send + Sync` as the state is (Compose's `State<T>` of a mutable state,
/// for what must be shareable — a shape, a theme value — and reads a state another part owns and sets: an
/// `Animatable`'s value, say). Reading it in a composable (or a measure, a draw) subscribes the reader as reading the
/// state does. Equal when it views the same state.
pub struct SharedState<T> {
    state: MutableState<T>,
}

impl<T> Clone for SharedState<T> {
    fn clone(&self) -> Self {
        SharedState { state: self.state.clone() }
    }
}

impl<T: Clone + Send + Sync + 'static> SharedState<T> {
    /// The value the snapshot this runs in sees (a read that subscribes; Compose's `value`).
    pub fn get(&self) -> T {
        self.state.get()
    }

    /// Runs `f` with the value, without copying it.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        self.state.with(f)
    }

    /// The value, without subscribing the reader.
    pub fn peek(&self) -> T {
        self.state.peek()
    }

    /// As a [`State`] (what a composable reading a value takes).
    pub fn as_state(&self) -> State<T> {
        State::from_source(self.state.clone())
    }
}

impl<T> PartialEq for SharedState<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state.cell, &other.state.cell)
    }
}

impl<T: Clone + Send + Sync + fmt::Debug + 'static> fmt::Debug for SharedState<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SharedState({:?})", self.peek())
    }
}

/// What a read-only state reads from.
pub trait StateSource<T>: 'static {
    /// Runs `f` with the value, the read recorded if `subscribe`.
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&T));
    /// Who it is: states are the same when their sources are.
    fn identity(&self) -> *const ();
}

impl<T: Clone + Send + Sync + 'static> StateSource<T> for MutableState<T> {
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&T)) {
        if subscribe { self.with(|v| f(v)) } else { self.peek_with(|v| f(v)) }
    }
    fn identity(&self) -> *const () {
        Arc::as_ptr(&self.cell) as *const ()
    }
}

/// A state that can be read but not set (Compose's `State<T>`): what a composable that only reads a value
/// takes, given a [`MutableState`] or a derived state.
pub struct State<T> {
    source: std::rc::Rc<dyn StateSource<T>>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        State { source: self.source.clone() }
    }
}

impl<T: Clone + 'static> State<T> {
    pub fn from_source(source: impl StateSource<T>) -> State<T> {
        State { source: std::rc::Rc::new(source) }
    }
    /// A state reading a source already shared.
    pub(crate) fn from_shared(source: std::rc::Rc<dyn StateSource<T>>) -> State<T> {
        State { source }
    }
    pub fn get(&self) -> T {
        self.with(T::clone)
    }
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        read_with(&*self.source, true, f)
    }
    /// Without subscribing.
    pub fn peek(&self) -> T {
        read_with(&*self.source, false, T::clone)
    }
    /// Its value (Compose's `state.value`): the same as [`get`](State::get).
    pub fn value(&self) -> T {
        self.get()
    }
    /// Who it is: its source's identity.
    pub(crate) fn identity(&self) -> *const () {
        self.source.identity()
    }
}

fn read_with<T: 'static, R>(source: &dyn StateSource<T>, subscribe: bool, f: impl FnOnce(&T) -> R) -> R {
    let mut f = Some(f);
    let mut out = None;
    source.with_value(subscribe, &mut |v| out = Some((f.take().unwrap())(v)));
    out.expect("a state source gave no value")
}

/// The same state, not the same value.
impl<T: 'static> PartialEq for State<T> {
    fn eq(&self, other: &Self) -> bool {
        self.source.identity() == other.source.identity()
    }
}

impl<T: Clone + Send + Sync + 'static> From<MutableState<T>> for State<T> {
    fn from(state: MutableState<T>) -> State<T> {
        State::from_source(state)
    }
}

impl<T: Clone + Send + Sync + 'static> From<&MutableState<T>> for State<T> {
    fn from(state: &MutableState<T>) -> State<T> {
        State::from_source(state.clone())
    }
}

impl<T: fmt::Display + Clone + 'static> fmt::Display for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.with(|v| v.fmt(f))
    }
}
