//! Who is told about a read: the innermost observer this thread installed (a composition's running scope, a
//! layout's measure, a drawing), as reads happen.

use crate::snapshot::StateKey;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static OBSERVERS: RefCell<Vec<Option<Rc<dyn Fn(StateKey)>>>> = const { RefCell::new(Vec::new()) };
}

/// Tells the innermost read observer that `key` was read. States call it as they are read (not peeked).
pub fn record_read(key: StateKey) {
    let top = OBSERVERS.with(|o| o.borrow().last().cloned().flatten());
    if let Some(observer) = top {
        observer(key);
    }
}

struct Pop;
impl Drop for Pop {
    fn drop(&mut self) {
        OBSERVERS.with(|o| o.borrow_mut().pop());
    }
}

/// Runs `f` with what it reads told to `observer` (and not to the observers outside it).
pub fn observe_reads<R>(observer: Rc<dyn Fn(StateKey)>, f: impl FnOnce() -> R) -> R {
    OBSERVERS.with(|o| o.borrow_mut().push(Some(observer)));
    let _pop = Pop;
    f()
}

/// Runs `f` with what it reads told to no one (Compose's Snapshot.withoutReadObservation).
pub fn without_read_observation<R>(f: impl FnOnce() -> R) -> R {
    OBSERVERS.with(|o| o.borrow_mut().push(None));
    let _pop = Pop;
    f()
}
