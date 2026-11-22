//! What composables call: remember, keys, restartable scopes, effects.

use crate::composer::{Composer, RememberObserver, key_of, mix_key, stable_hash};
use crate::kompozable;
use std::any::Any;
use std::cell::Cell;
use std::hash::Hash;
use std::panic::Location;
use std::rc::Rc;

/// Runs `f` on what the current group remembered, if it is a `T` (taken out meanwhile: see `take_slot`).
fn slot_get<T: 'static, R>(c: &Composer, f: impl FnOnce(&mut T) -> Option<R>) -> Option<R> {
    let mut slot = c.take_slot()?;
    let r = slot.value.downcast_mut::<T>().and_then(f);
    c.put_slot(slot);
    r
}

/// `remember { … }`: computes the value in the first composition and gives the same one afterwards (a copy of
/// it: states and `Rc`s share what they hold).
#[track_caller]
pub fn remember<T: Clone + 'static>(init: impl FnOnce() -> T) -> T {
    let c = Composer::require();
    c.start_group(key_of(Location::caller()));
    let value = match slot_get::<T, _>(&c, |v| Some(v.clone())) {
        Some(v) => v,
        None => {
            let v = init();
            c.set_remembered(Box::new(v.clone()), None);
            v
        }
    };
    c.end_group();
    value
}

/// `remember(keys) { … }`: computed again whenever `keys` change (several: a tuple).
#[track_caller]
pub fn remember_keyed<K: PartialEq + 'static, T: Clone + 'static>(keys: K, init: impl FnOnce() -> T) -> T {
    let c = Composer::require();
    c.start_group(key_of(Location::caller()));
    let kept = slot_get::<(K, T), _>(&c, |(k, v)| (*k == keys).then(|| v.clone()));
    let value = match kept {
        Some(v) => v,
        None => {
            let v = init();
            c.set_remembered(Box::new((keys, v.clone())), None);
            v
        }
    };
    c.end_group();
    value
}

/// `remember { … }` of a value told when it enters and leaves the composition ([`RememberObserver`]).
#[track_caller]
pub fn remember_observer<T: RememberObserver + 'static>(init: impl FnOnce() -> T) -> Rc<T> {
    let c = Composer::require();
    c.start_group(key_of(Location::caller()));
    let value = match slot_get::<Rc<T>, _>(&c, |v| Some(v.clone())) {
        Some(v) => v,
        None => {
            let v = Rc::new(init());
            c.set_remembered(Box::new(v.clone()), Some(v.clone()));
            v
        }
    };
    c.end_group();
    value
}

/// `remember(keys) { … }` of a value told when it enters and leaves the composition: made again (the old one
/// forgotten) when `keys` change.
#[track_caller]
pub fn remember_observer_keyed<K: PartialEq + 'static, T: RememberObserver + 'static>(
    keys: K,
    init: impl FnOnce() -> T,
) -> Rc<T> {
    let c = Composer::require();
    c.start_group(key_of(Location::caller()));
    let kept = slot_get::<(K, Rc<T>), _>(&c, |(k, v)| (*k == keys).then(|| v.clone()));
    let value = match kept {
        Some(v) => v,
        None => {
            let v = Rc::new(init());
            c.set_remembered(Box::new((keys, v.clone())), Some(v.clone()));
            v
        }
    };
    c.end_group();
    value
}

thread_local! {
    /// The last memoized closure identity given out (identities are unique on their thread, the UI's).
    static LAST_ID: Cell<u64> = const { Cell::new(0) };
}

fn new_id() -> u64 {
    LAST_ID.with(|l| {
        l.set(l.get() + 1);
        l.get()
    })
}

/// The identity of a closure memoized at `site` by `keys` (see `__private::memo_id`).
pub(crate) fn memo_id_at<K: crate::private::Keys + 'static>(site: &Location<'_>, keys: K) -> Option<u64> {
    let c = Composer::current()?;
    if !c.in_group() {
        return None;
    }
    c.start_group(key_of(site));
    let kept = slot_get::<(K, u64), _>(&c, |(k, id)| k.same(&keys).then_some(*id));
    let id = match kept {
        Some(id) => id,
        None => {
            let id = new_id();
            c.set_remembered(Box::new((keys, id)), None);
            id
        }
    };
    c.end_group();
    Some(id)
}

/// A `#[memoized]` parameter: the value given and its identity as a memoized closure (Compose's strong skipping
/// remembers a lambda by what it captures): the same identity while what the closure captures stays the same,
/// none when it wasn't one (or outside a composition).
pub struct Memoized<T> {
    value: T,
    id: Option<u64>,
}

impl<T> Memoized<T> {
    pub fn new(value: T, id: Option<u64>) -> Memoized<T> {
        Memoized { value, id }
    }

    /// Its identity, if it has one.
    pub fn id(&self) -> Option<u64> {
        self.id
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn into_value(self) -> T {
        self.value
    }

    /// What `make` makes of it, remembered while its identity stays the same (made again from a closure that
    /// captures other values, or from a value without an identity): `Layout` keeps its measure policy so.
    #[track_caller]
    pub fn remember<R: Clone + 'static>(self, make: impl FnOnce(T) -> R) -> R {
        let Some(c) = Composer::current().filter(|c| c.in_group()) else { return make(self.value) };
        c.start_group(key_of(Location::caller()));
        let id = self.id;
        let kept = slot_get::<(Option<u64>, R), _>(&c, |(k, v)| (id.is_some() && *k == id).then(|| v.clone()));
        let value = match kept {
            Some(v) => v,
            None => {
                let v = make(self.value);
                c.set_remembered(Box::new((id, v.clone())), None);
                v
            }
        };
        c.end_group();
        value
    }
}

/// Gives `content` an explicit identity, e.g. for list items that can move or be removed: its state goes with
/// the key, not with its position.
#[track_caller]
pub fn key<K: Hash + ?Sized, R>(id: &K, content: impl FnOnce() -> R) -> R {
    let c = Composer::require();
    c.start_group(mix_key(key_of(Location::caller()), stable_hash(id)));
    let r = content();
    c.end_group();
    r
}

/// A group of its own around `content`, without a scope: an identity for a composable that runs its content
/// in place (what Compose's inline composables, `Column`, `Row`, get from the compiler).
#[track_caller]
pub fn group<R>(content: impl FnOnce() -> R) -> R {
    let c = Composer::require();
    c.start_group(key_of(Location::caller()));
    let r = content();
    c.end_group();
    r
}

/// Emits a node (what a layout composable does): made by `factory` the first time, configured by `update` at
/// every call, its children emitted by `content`.
#[track_caller]
pub fn emit<N: crate::Node>(factory: impl FnOnce() -> Rc<N>, update: impl FnOnce(&N), content: impl FnOnce()) {
    let c = Composer::require();
    c.emit(
        key_of(Location::caller()),
        || factory() as Rc<dyn crate::Node>,
        |n| update((&**n as &dyn Any).downcast_ref::<N>().expect("a node of another type in its group")),
        content,
    );
}

/// A restartable, skippable composable: what a plain `@Composable` function is. `body` runs in a scope of its
/// own: it runs again on its own when a state it read changes, and isn't run when `args` equal the last ones
/// (`equals`) and nothing it read changed. Without `equals`, restartable but never skipped.
#[track_caller]
pub fn restartable<A: 'static>(args: A, equals: Option<fn(&A, &A) -> bool>, body: impl Fn(&A) + 'static) {
    restartable_at(key_of(Location::caller()), args, equals, body);
}

pub(crate) fn restartable_at<A: 'static>(
    key: u64,
    args: A,
    equals: Option<fn(&A, &A) -> bool>,
    body: impl Fn(&A) + 'static,
) {
    let c = Composer::require();
    let eq = equals.map(|_| compare::<A> as fn(&dyn Any, &dyn Any) -> bool);
    let args: Rc<dyn Any> = Rc::new(Args { value: args, equals });
    let body: Rc<dyn Fn(&dyn Any)> =
        Rc::new(move |a: &dyn Any| body(&a.downcast_ref::<Args<A>>().expect("arguments").value));
    c.restartable(key, args, eq, body);
}

/// A scope's arguments, with how they compare.
struct Args<A> {
    value: A,
    equals: Option<fn(&A, &A) -> bool>,
}

fn compare<A: 'static>(a: &dyn Any, b: &dyn Any) -> bool {
    match (a.downcast_ref::<Args<A>>(), b.downcast_ref::<Args<A>>()) {
        (Some(a), Some(b)) => a.equals.is_some_and(|eq| eq(&a.value, &b.value)),
        _ => false,
    }
}

/// Content in a restartable scope of its own (Compose's composable lambdas): when only what it read changes, it
/// alone runs again. It can't be compared, so when its caller runs, it runs.
#[track_caller]
pub fn scoped(content: impl Fn() + 'static) {
    restartable((), None, move |_| content());
}

/// Runs after every successful composition of the enclosing scope (after what entered the composition was told
/// so).
#[kompozable(inline)]
pub fn SideEffect(effect: impl FnOnce() + 'static) {
    Composer::require().side_effect(Box::new(effect));
}

/// What a [`DisposableEffect`] gives back: its cleanup ([`on_dispose`]).
pub struct DisposableEffectResult(Box<dyn FnOnce()>);

/// The cleanup of a [`DisposableEffect`], run when it leaves the composition or runs again.
pub fn on_dispose(cleanup: impl FnOnce() + 'static) -> DisposableEffectResult {
    DisposableEffectResult(Box::new(cleanup))
}

/// Runs `effect` after the composition when it enters the composition or when `keys` change (several: a tuple);
/// the cleanup it returns runs when it leaves or runs again.
#[kompozable(inline)]
pub fn DisposableEffect<K: PartialEq + 'static>(keys: K, effect: impl FnOnce() -> DisposableEffectResult + 'static) {
    let c = Composer::require();
    let g = c.current_group();
    let mut keys = Some(keys);
    let changed = slot_get::<K, _>(&c, |k| {
        if *k == *keys.as_ref().unwrap() {
            Some(false)
        } else {
            *k = keys.take().unwrap();
            Some(true)
        }
    });
    let pending = match changed {
        Some(changed) => changed,
        None => {
            c.set_remembered(Box::new(keys.take().unwrap()), None);
            true
        }
    };
    if pending {
        c.replace_on_forget(); // the old one's, before any new effect runs
        let composer = Rc::downgrade(&c);
        c.after_composition(Box::new(move || {
            let cleanup = effect().0;
            if let Some(c) = composer.upgrade() {
                c.set_on_forget(g, cleanup);
            } else {
                cleanup();
            }
        }));
    }
}

/// `rememberUpdatedState(newValue)`: a state set to the value at every composition — Compose's
/// `remember { mutableStateOf(newValue) }.apply { value = newValue }`, a read-only [`State`](crate::State). A
/// long-running effect or a remembered callback reads the newest value through it without restarting; a composition,
/// a layout or a drawing that reads it is invalidated when a composition sets it to a value that isn't equal to the
/// one it has.
///
/// Equal is Kotlin's `==`, as Compose's structural equality policy: the value's `==` if its type has one, else the
/// same instance for a shared handle (an `Rc`, an `Arc`: a callback `Rc<dyn Fn…>` is equal only to itself, as a
/// Kotlin lambda is), else never (a closure, a type without `==`: every composition counts as a change, as an
/// unequal lambda instance does in Compose). The value's type is read where the macro is written, so it must be
/// known there (`remember_updated_state!(Rc::new(on_tap) as Rc<dyn Fn(Offset)>)`).
///
/// Compose's lambdas are memoized (the same instance while what they capture stays the same); a closure here, or an
/// `Rc` made anew at each composition (`Rc::new(on_click)` of an `impl Fn` parameter), is a change every time the
/// composition setting it runs: what reads it in a measure or a drawing runs again then, and a composition that
/// reads it where it is set (itself, or through a derived state of it) would be composed again at every pass. Read
/// such a callback in effects and callbacks, as Compose's samples do, or take it as an `Rc<dyn Fn…>` parameter: a
/// scope restarting by itself is given the same instance again, equal to itself.
///
/// ```
/// # use kompoz_runtime::*;
/// # use std::rc::Rc;
/// #[kompozable]
/// fn Timer(on_timeout: impl Fn() + 'static) {
///     let current_on_timeout = remember_updated_state!(Rc::new(on_timeout) as Rc<dyn Fn()>);
///     DisposableEffect((), move || {
///         // A timer started once: when it fires, it calls the callback given last.
///         let _fire = move || (current_on_timeout.get())();
///         on_dispose(|| {})
///     });
/// }
/// ```
///
/// Kotlin: `rememberUpdatedState(newValue: T): State<T>`
#[macro_export]
macro_rules! __remember_updated_state {
    ($value:expr $(,)?) => {{
        #[allow(unused_imports)]
        use $crate::__private::{EqFallback as _, EqViaInstance as _, EqViaPartialEq as _};
        let __kz_value = $value;
        let __kz_equivalent = (&&&$crate::__private::EqProbe::of(&__kz_value)).__kz_eq();
        $crate::__private::remember_updated_state_with(__kz_value, __kz_equivalent)
    }};
}

/// What [`remember_updated_state!`](crate::remember_updated_state) keeps: the latest value, and the state its
/// readers read. Compose keeps the value in the snapshot state itself; a value here needn't be `Send` (a callback
/// rarely is) and KompozR's snapshot states hold `Send` values, so it is held beside a state of its changes, whose
/// reads and writes go through the snapshot system as the value's would (the readers recorded, a derived state's
/// dependency, told when the composition that set it is applied). The value itself isn't kept per snapshot: only
/// its composition sets it, on the UI thread, and what reads it there runs after that composition's snapshot is
/// applied, so it sees what a snapshot state would (a snapshot taken before, and read after, sees the new value).
struct UpdatedValue<T> {
    value: std::cell::RefCell<T>,
    changes: crate::MutableState<u64>,
    equivalent: Option<fn(&T, &T) -> bool>,
}

impl<T> UpdatedValue<T> {
    /// `value`, unless it is equal to the one it has: a change its readers are told of.
    fn set(&self, value: T) {
        if self.equivalent.is_some_and(|eq| eq(&self.value.borrow(), &value)) {
            return;
        }
        let old = self.value.replace(value);
        drop(old);
        self.changes.set(self.changes.peek().wrapping_add(1));
    }
}

impl<T: 'static> crate::StateSource<T> for UpdatedValue<T> {
    fn with_value(&self, subscribe: bool, f: &mut dyn FnMut(&T)) {
        if subscribe {
            self.changes.with(|_| ());
        }
        f(&self.value.borrow());
    }
    fn identity(&self) -> *const () {
        self as *const Self as *const ()
    }
}

/// [`remember_updated_state!`](crate::remember_updated_state) with the equality its value's type has.
#[doc(hidden)]
#[track_caller]
pub fn remember_updated_state_with<T: Clone + 'static>(
    value: T,
    equivalent: Option<fn(&T, &T) -> bool>,
) -> crate::State<T> {
    let site = Location::caller();
    let c = Composer::require();
    c.start_group(key_of(site));
    let holder = match slot_get::<Rc<UpdatedValue<T>>, _>(&c, |h| Some(h.clone())) {
        Some(h) => {
            h.set(value);
            h
        }
        None => {
            let h = Rc::new(UpdatedValue {
                value: std::cell::RefCell::new(value),
                changes: crate::mutable_state_of(0),
                equivalent,
            });
            c.set_remembered(Box::new(h.clone()), None);
            h
        }
    };
    c.end_group();
    crate::State::from_shared(holder)
}

/// The scope running now composed again at the next frame (an animation's).
pub fn request_frame() {
    if let Some(c) = Composer::current() {
        c.request_frame();
    }
}

/// The scope running now composed again at the frame at `frame_time_nanos`, or the first after.
pub fn request_frame_at(frame_time_nanos: i64) {
    if let Some(c) = Composer::current() {
        c.request_frame_at(frame_time_nanos);
    }
}

/// The time of the frame being composed (what `request_frame` scopes compose at).
pub fn frame_time_nanos() -> i64 {
    Composer::current().map_or(0, |c| c.frame_time())
}

/// The key of where this runs in the composition (Compose's `currentCompositeKeyHash`): the same at the same place
/// whenever it composes again, so state can be found again by it.
pub fn current_composite_key_hash() -> u64 {
    Composer::require().key_below(None)
}

/// Whether this runs in a composition.
pub fn in_composition() -> bool {
    Composer::current().is_some()
}
