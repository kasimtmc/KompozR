//! Work for the time between frames: what the host does in the pause before the next frame is due
//! ([`KompozHost::idle`](crate::KompozHost::idle)), as Compose's prefetch scheduler does on Android between the
//! Choreographer's frames — a lazy layout's next item composed before it scrolls in. Each owner has one piece of
//! work waiting at a time; a piece runs while there is time ([`IdleScope::available_time_nanos`]) and says whether
//! it has more to do (it is run again in the next pause, before what was asked after it).

use kompoz_runtime::KompozitionLocal;
use kompoz_runtime::clock::Instant;
use std::cell::RefCell;
use std::rc::Rc;

/// What a piece of idle work is told (Compose's PrefetchRequestScope): how long it has.
pub struct IdleScope {
    deadline: Instant,
}

impl IdleScope {
    /// A scope whose time ends at `deadline`.
    pub fn new(deadline: Instant) -> IdleScope {
        IdleScope { deadline }
    }

    /// The time left before the next frame is due, ns (Compose's `availableTimeNanos`; 0 once it is).
    pub fn available_time_nanos(&self) -> i64 {
        self.deadline.saturating_duration_since(Instant::now()).as_nanos().min(i64::MAX as u128) as i64
    }
}

/// A piece of work: given the time it has, whether it has more to do.
pub type IdleTask = Box<dyn FnMut(&IdleScope) -> bool>;

/// What waits for a pause: C++'s one-shot form (an estimate, a change made at once, the time it took told), or a
/// task run as long as it says it has more.
pub(crate) enum IdleWork {
    Once { estimate_nanos: i64, start: Box<dyn FnOnce() -> bool>, took: Box<dyn FnOnce(i64)> },
    Task(IdleTask),
}

pub(crate) struct IdleEntry {
    pub(crate) owner: usize,
    pub(crate) work: IdleWork,
}

/// The work waiting for the pauses between a host's frames, in the order asked, one for each owner (a later request
/// of an owner replaces its earlier one). The host provides it to its composition ([`LocalIdleWorkQueue`]) and runs
/// it in [`KompozHost::idle`](crate::KompozHost::idle). Clones are the same queue.
#[derive(Clone, Default)]
pub struct IdleWorkQueue {
    entries: Rc<RefCell<Vec<IdleEntry>>>,
}

impl PartialEq for IdleWorkQueue {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.entries, &other.entries)
    }
}

impl IdleWorkQueue {
    pub fn new() -> IdleWorkQueue {
        IdleWorkQueue::default()
    }

    /// Asks for `task` to run in the pauses between frames, for `owner` (an address that stays the owner's: a later
    /// request replaces this one): it runs while there is time, returning whether it has more to do (then it runs
    /// again in the next pause, first).
    pub fn request(&self, owner: usize, task: impl FnMut(&IdleScope) -> bool + 'static) {
        self.put(IdleEntry { owner, work: IdleWork::Task(Box::new(task)) });
    }

    /// Whether `owner` has work waiting.
    pub fn has_work_of(&self, owner: usize) -> bool {
        self.entries.borrow().iter().any(|e| e.owner == owner)
    }

    /// Drops `owner`'s work, if any waits.
    pub fn cancel(&self, owner: usize) {
        self.entries.borrow_mut().retain(|e| e.owner != owner);
    }

    /// Whether any work waits.
    pub fn has_work(&self) -> bool {
        !self.entries.borrow().is_empty()
    }

    pub(crate) fn put(&self, entry: IdleEntry) {
        let mut entries = self.entries.borrow_mut();
        match entries.iter_mut().find(|e| e.owner == entry.owner) {
            Some(e) => *e = entry,
            None => entries.push(entry),
        }
    }

    /// The first piece of work, taken out to run, if `fits` says it may run now (its estimate within the time left).
    pub(crate) fn take_first(&self, fits: impl FnOnce(&IdleWork) -> bool) -> Option<IdleEntry> {
        let mut entries = self.entries.borrow_mut();
        match entries.first() {
            Some(e) if fits(&e.work) => Some(entries.remove(0)),
            _ => None,
        }
    }

    /// Puts a task that has more to do back first, unless its owner asked for something else meanwhile.
    pub(crate) fn put_back_first(&self, entry: IdleEntry) {
        let mut entries = self.entries.borrow_mut();
        if !entries.iter().any(|e| e.owner == entry.owner) {
            entries.insert(0, entry);
        }
    }
}

/// The idle work queue of the host the composition is in (none outside a host: then there are no pauses to run
/// work in, as Compose's prefetch is off where it has no scheduler).
pub static LocalIdleWorkQueue: KompozitionLocal<Option<IdleWorkQueue>> = KompozitionLocal::new_static(|| None);
