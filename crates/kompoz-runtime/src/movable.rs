//! Movable content: content whose state goes with it wherever it is composed next.

use crate::composer::{Composer, MovableData};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Content whose state goes with it wherever it is composed next (Compose's movableContentOf): called from
/// another place in the same composition (a `Row` instead of a `Column`, another pane), its remembered values,
/// nodes and running effects move there instead of starting over. Called in two places at once, the second is
/// a new instance. Keep it with `remember`:
///
/// ```
/// # use kompoz_runtime::*;
/// # fn content(wide: bool) {
/// let video = remember(|| movable_content_of(|| { /* VideoPlayer() */ }));
/// if wide { /* Row */ video.call() } else { /* Column */ video.call() }
/// # }
/// ```
#[derive(Clone)]
pub struct MovableContent {
    data: Rc<MovableData>,
}

impl MovableContent {
    pub fn new(content: impl Fn() + 'static) -> Self {
        MovableContent {
            data: Rc::new(MovableData { content: RefCell::new(Some(Rc::new(content))), group: Cell::new(None) }),
        }
    }

    /// Composes it here.
    pub fn call(&self) {
        Composer::require().compose_movable(&self.data);
    }
}

impl PartialEq for MovableContent {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.data, &other.data)
    }
}

pub fn movable_content_of(content: impl Fn() + 'static) -> MovableContent {
    MovableContent::new(content)
}

/// The same taking a parameter (Compose's `movableContentOf { p -> … }`; several: a tuple): each call composes
/// it with that, its state going with it.
pub struct MovableContentOf<P> {
    inner: MovableContent,
    args: Rc<RefCell<Option<Rc<P>>>>,
}

impl<P> Clone for MovableContentOf<P> {
    fn clone(&self) -> Self {
        MovableContentOf { inner: self.inner.clone(), args: self.args.clone() }
    }
}

impl<P: 'static> MovableContentOf<P> {
    pub fn new(content: impl Fn(&P) + 'static) -> Self {
        let args: Rc<RefCell<Option<Rc<P>>>> = Rc::new(RefCell::new(None));
        let held = args.clone();
        let inner = MovableContent::new(move || {
            let p = held.borrow().clone(); // the latest: what it composes with
            if let Some(p) = p {
                content(&p);
            }
        });
        MovableContentOf { inner, args }
    }

    /// Composes it here with `p`.
    pub fn call(&self, p: P) {
        *self.args.borrow_mut() = Some(Rc::new(p)); // what it composes with here
        self.inner.call();
    }
}

impl<P> PartialEq for MovableContentOf<P> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

pub fn movable_content_with_param_of<P: 'static>(content: impl Fn(&P) + 'static) -> MovableContentOf<P> {
    MovableContentOf::new(content)
}
