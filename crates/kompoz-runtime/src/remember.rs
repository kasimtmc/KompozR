//! What composables call: remember, keys, restartable scopes, effects.

use crate::composer::{Composer, RememberObserver, key_of, mix_key, stable_hash};
use crate::kompozable;
use std::any::Any;
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

/// The latest value passed to [`remember_updated_state`]. Not observable: meant for long-running effects that
/// must not restart but should use the newest callback or value.
pub struct UpdatedState<T>(Rc<std::cell::RefCell<T>>);

impl<T> Clone for UpdatedState<T> {
    fn clone(&self) -> Self {
        UpdatedState(self.0.clone())
    }
}

impl<T: Clone> UpdatedState<T> {
    pub fn get(&self) -> T {
        self.0.borrow().clone()
    }
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.0.borrow())
    }
}

#[track_caller]
pub fn remember_updated_state<T: 'static>(value: T) -> UpdatedState<T> {
    let site = Location::caller();
    let c = Composer::require();
    c.start_group(key_of(site));
    let holder = match slot_get::<Rc<std::cell::RefCell<T>>, _>(&c, |h| Some(h.clone())) {
        Some(h) => {
            *h.borrow_mut() = value;
            h
        }
        None => {
            let h = Rc::new(std::cell::RefCell::new(value));
            c.set_remembered(Box::new(h.clone()), None);
            h
        }
    };
    c.end_group();
    UpdatedState(holder)
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

/// Whether this runs in a composition.
pub fn in_composition() -> bool {
    Composer::current().is_some()
}
