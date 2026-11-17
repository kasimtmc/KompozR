//! Pinned items (Compose's LazyLayoutPinnableItem.kt): an item is the pinnable container of its content
//! (`LocalPinnableContainer`); content that pins it (a focused text field scrolled away) keeps it composed and
//! laid out while it is out of view, until it releases the pin.

use super::key::LazyKey;
use kompoz_runtime::{
    DisposableEffect, KompozitionLocalProvider, MutableState, kompozable, mutable_state_of, on_dispose, remember_keyed,
};
use kompoz_ui::layout::{LocalPinnableContainer, PinnableContainer, PinnableContainerRef, PinnedHandle};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// The items pinned in a lazy layout (Compose's LazyLayoutPinnedItemList): observed as a snapshot list is (the
/// layout measures again as it changes). Clones are the same list.
#[derive(Clone)]
pub struct LazyLayoutPinnedItemList {
    items: Rc<RefCell<Vec<PinnedItemRef>>>,
    /// Changed with the items: what reads them is told.
    version: MutableState<u64>,
}

impl Default for LazyLayoutPinnedItemList {
    fn default() -> Self {
        LazyLayoutPinnedItemList { items: Rc::new(RefCell::new(Vec::new())), version: mutable_state_of(0) }
    }
}

impl PartialEq for LazyLayoutPinnedItemList {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.items, &other.items)
    }
}

/// A pinned item (Compose's `LazyLayoutPinnedItemList.PinnedItem`): its key and its index when last composed.
pub trait PinnedItem {
    fn key(&self) -> Option<LazyKey>;
    fn index(&self) -> i32;
}

/// A pinned item as the list holds it: compared by identity.
#[derive(Clone)]
pub struct PinnedItemRef(Rc<LazyLayoutPinnableItem>);

impl PartialEq for PinnedItemRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl PinnedItem for PinnedItemRef {
    fn key(&self) -> Option<LazyKey> {
        self.0.key.clone()
    }
    fn index(&self) -> i32 {
        self.0.index.get()
    }
}

impl LazyLayoutPinnedItemList {
    pub fn new() -> LazyLayoutPinnedItemList {
        LazyLayoutPinnedItemList::default()
    }

    fn changed(&self) {
        self.version.set(self.version.peek().wrapping_add(1));
    }

    fn pin(&self, item: PinnedItemRef) {
        self.items.borrow_mut().push(item);
        self.changed();
    }

    fn release(&self, item: &PinnedItemRef) {
        let removed = {
            let mut items = self.items.borrow_mut();
            items.iter().position(|i| i == item).map(|at| items.remove(at))
        };
        if removed.is_some() {
            self.changed();
        }
    }

    /// The pinned items now (a state read).
    pub fn to_list(&self) -> Vec<PinnedItemRef> {
        self.version.get();
        self.items.borrow().clone()
    }

    /// How many are pinned (a state read).
    pub fn len(&self) -> usize {
        self.version.get();
        self.items.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Composes `content` as the item `index` of `key`, a pinnable container its content can pin to keep it composed
/// (Compose's `LazyLayoutPinnableItem`): pinned, it is in `pinned_item_list`, and it pins the container it is in.
#[kompozable]
pub fn LazyLayoutPinnableItem(
    key: Option<LazyKey>,
    index: i32,
    pinned_item_list: LazyLayoutPinnedItemList,
    content: impl Fn() + 'static,
) {
    let list = pinned_item_list.clone();
    let k = key.clone();
    let pinnable_item = remember_keyed((key, pinned_item_list), move || LazyLayoutPinnableItem::new(k, list));
    pinnable_item.index.set(index);
    pinnable_item.set_parent_pinnable_container(LocalPinnableContainer.current());
    let item = pinnable_item.clone();
    DisposableEffect(PinnedItemRef(pinnable_item.clone()), move || on_dispose(move || item.on_disposed()));
    let container = PinnableContainerRef(pinnable_item.clone() as Rc<dyn PinnableContainer>);
    KompozitionLocalProvider(&[LocalPinnableContainer.provides(Some(container))], content);
}

/// An item that can be pinned (Compose's LazyLayoutPinnableItem): counting its pins, in the list while pinned,
/// holding a pin of its parent container meanwhile.
pub struct LazyLayoutPinnableItem {
    key: Option<LazyKey>,
    pinned_item_list: LazyLayoutPinnedItemList,
    index: Cell<i32>,
    pins_count: Cell<i32>,
    parent_handle: RefCell<Option<PinnedHandle>>,
    is_disposed: Cell<bool>,
    /// The container it is in.
    parent_pinnable_container: RefCell<Option<PinnableContainerRef>>,
    me: Weak<LazyLayoutPinnableItem>,
}

impl LazyLayoutPinnableItem {
    fn new(key: Option<LazyKey>, pinned_item_list: LazyLayoutPinnedItemList) -> Rc<LazyLayoutPinnableItem> {
        Rc::new_cyclic(|me| LazyLayoutPinnableItem {
            key,
            pinned_item_list,
            index: Cell::new(-1),
            pins_count: Cell::new(0),
            parent_handle: RefCell::new(None),
            is_disposed: Cell::new(false),
            parent_pinnable_container: RefCell::new(None),
            me: me.clone(),
        })
    }

    fn me(&self) -> PinnedItemRef {
        PinnedItemRef(self.me.upgrade().expect("a pinnable item"))
    }

    fn parent_pinnable_container(&self) -> Option<PinnableContainerRef> {
        self.parent_pinnable_container.borrow().clone()
    }

    /// The container it is in: a pin held moves to the new one.
    fn set_parent_pinnable_container(&self, value: Option<PinnableContainerRef>) {
        let previous = self.parent_pinnable_container();
        if value != previous {
            *self.parent_pinnable_container.borrow_mut() = value.clone();
            if self.pins_count.get() > 0 {
                if let Some(h) = self.parent_handle.borrow_mut().take() {
                    h.release();
                }
                *self.parent_handle.borrow_mut() = value.map(|v| v.pin());
            }
        }
    }

    fn unpin(&self) {
        self.pinned_item_list.release(&self.me());
        if let Some(h) = self.parent_handle.borrow_mut().take() {
            h.release();
        }
    }

    /// A pin released (Compose's `release`): the last one unpins it.
    fn release(&self) {
        if self.is_disposed.get() {
            return; // (already as the item is disposed of)
        }
        assert!(self.pins_count.get() > 0, "Release should only be called once");
        self.pins_count.set(self.pins_count.get() - 1);
        if self.pins_count.get() == 0 {
            self.unpin();
        }
    }

    fn on_disposed(&self) {
        self.is_disposed.set(true);
        self.pins_count.set(0);
        self.unpin();
    }
}

impl PinnableContainer for LazyLayoutPinnableItem {
    fn pin(&self) -> PinnedHandle {
        assert!(!self.is_disposed.get(), "Pin should not be called on an already disposed item ");
        if self.pins_count.get() == 0 {
            self.pinned_item_list.pin(self.me());
            *self.parent_handle.borrow_mut() = self.parent_pinnable_container().map(|p| p.pin());
        }
        self.pins_count.set(self.pins_count.get() + 1);
        let me = self.me.clone();
        PinnedHandle::new(move || {
            if let Some(me) = me.upgrade() {
                me.release();
            }
        })
    }
}
