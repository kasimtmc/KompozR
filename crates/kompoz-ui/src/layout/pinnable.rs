//! Pinning (Compose's PinnableContainer.kt): content that matters now (a focused item) asks the container it is in
//! (a lazy list's item) to keep it composed while it is out of view.

use kompoz_runtime::KompozitionLocal;
use std::rc::Rc;

/// A container its content can pin (Compose's PinnableContainer): a lazy layout's item keeps what is pinned
/// composed and laid out while it is scrolled out of view.
pub trait PinnableContainer {
    /// Pins it; the handle releases the pin (Compose's `pin`).
    fn pin(&self) -> PinnedHandle;
}

/// A pin, until [`release`](PinnedHandle::release)d (Compose's PinnableContainer.PinnedHandle).
#[derive(Clone)]
pub struct PinnedHandle(Rc<dyn Fn()>);

impl PinnedHandle {
    /// A pin whose release calls `release`.
    pub fn new(release: impl Fn() + 'static) -> PinnedHandle {
        PinnedHandle(Rc::new(release))
    }

    /// Releases the pin: the container may let the content go again.
    pub fn release(&self) {
        (self.0)()
    }
}

/// A [`PinnableContainer`] as [`LocalPinnableContainer`] holds it: compared by identity.
#[derive(Clone)]
pub struct PinnableContainerRef(pub Rc<dyn PinnableContainer>);

impl PinnableContainerRef {
    /// Pins the container (see [`PinnableContainer::pin`]).
    pub fn pin(&self) -> PinnedHandle {
        self.0.pin()
    }
}

impl PartialEq for PinnableContainerRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The container what is composed here is in, none outside one (Compose's LocalPinnableContainer).
pub static LocalPinnableContainer: KompozitionLocal<Option<PinnableContainerRef>> = KompozitionLocal::new(|| None);
