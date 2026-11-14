//! Back handling (C++'s back handlers; Compose's BackHandler on Android's OnBackPressedDispatcher): the most recent
//! enabled handler gets back. Predictive back gestures and their scoping to focused popups are the windows package
//! of this stage.

use std::cell::Cell;
use std::rc::{Rc, Weak};

/// A back handler: what back does while it is enabled.
pub struct BackCallback {
    pub(crate) on_back: Box<dyn Fn()>,
    pub(crate) enabled: Cell<bool>,
}

impl BackCallback {
    pub fn new(on_back: impl Fn() + 'static) -> Rc<BackCallback> {
        Rc::new(BackCallback { on_back: Box::new(on_back), enabled: Cell::new(true) })
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }
}

/// The host's back handlers, oldest first.
#[derive(Default)]
pub struct BackDispatcher {
    handlers: Vec<Weak<BackCallback>>,
}

impl BackDispatcher {
    /// Adds a handler (held by its owner: gone with it).
    pub fn add(&mut self, callback: &Rc<BackCallback>) {
        self.handlers.push(Rc::downgrade(callback));
    }

    pub fn remove(&mut self, callback: &Rc<BackCallback>) {
        self.handlers.retain(|w| !w.ptr_eq(&Rc::downgrade(callback)));
    }

    /// The handler that would take back now: the most recent enabled one.
    pub fn active(&self) -> Option<Rc<BackCallback>> {
        self.handlers.iter().rev().filter_map(Weak::upgrade).find(|h| h.enabled.get())
    }

    /// Whether back would do something.
    pub fn wants_back(&self) -> bool {
        self.active().is_some()
    }

    /// Back pressed: the active handler runs. False if none took it (the platform does its default).
    pub fn back_invoked(&mut self) -> bool {
        self.handlers.retain(|w| w.strong_count() > 0);
        match self.active() {
            Some(h) => {
                (h.on_back)();
                true
            }
            None => false,
        }
    }
}
