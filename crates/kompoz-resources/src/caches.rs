//! Resource caches (CMP's ResourceCaches.kt): what was loaded — string records, images, vectors, fonts, plural
//! rules — kept by key for the process, each loaded once however many ask at once; [`ResourceCaches::clear`]
//! empties them all. And Kotlin's `runBlocking`, which desktop's composables load resources with
//! (ResourceState.blocking.kt).

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::pin::pin;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Wake, Waker};

/// An entry: loaded, or being loaded (who asks meanwhile waits for it).
enum Entry<V> {
    Loading(Arc<(Mutex<Option<Option<V>>>, Condvar)>),
    Loaded(V),
}

/// Values by key, each loaded once (CMP's `AsyncCache`): a load that fails isn't kept (Kotlin's cancelled or
/// failed `Deferred` is loaded again).
pub(crate) struct AsyncCache<K, V> {
    entries: Mutex<Option<HashMap<K, Entry<V>>>>,
}

impl<K: Eq + Hash + Clone, V: Clone> AsyncCache<K, V> {
    pub(crate) const fn new() -> AsyncCache<K, V> {
        AsyncCache { entries: Mutex::new(None) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<HashMap<K, Entry<V>>>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The value of `key`: kept, or loaded by `load` now (others asking meanwhile wait for it on their threads, as
    /// Kotlin's awaiting the same `Deferred`).
    pub(crate) async fn get_or_load<E, F: Future<Output = Result<V, E>>>(
        &self,
        key: K,
        load: impl FnOnce() -> F,
    ) -> Result<V, E> {
        let waiting = {
            let mut guard = self.lock();
            let map = guard.get_or_insert_with(HashMap::new);
            match map.get(&key) {
                Some(Entry::Loaded(v)) => return Ok(v.clone()),
                Some(Entry::Loading(slot)) => Some(slot.clone()),
                None => {
                    map.insert(key.clone(), Entry::Loading(Arc::new((Mutex::new(None), Condvar::new()))));
                    None
                }
            }
        };
        if let Some(slot) = waiting {
            let (lock, ready) = &*slot;
            let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());
            while done.is_none() {
                done = ready.wait(done).unwrap_or_else(|e| e.into_inner());
            }
            if let Some(Some(v)) = done.as_ref() {
                return Ok(v.clone());
            }
            // (it failed there: load it here)
        }
        let result = load().await;
        let mut guard = self.lock();
        let map = guard.get_or_insert_with(HashMap::new);
        let previous = match &result {
            Ok(v) => map.insert(key, Entry::Loaded(v.clone())),
            Err(_) => map.remove(&key),
        };
        if let Some(Entry::Loading(slot)) = previous {
            let (lock, ready) = &*slot;
            *lock.lock().unwrap_or_else(|e| e.into_inner()) = Some(result.as_ref().ok().cloned());
            ready.notify_all();
        }
        result
    }

    /// Everything kept goes.
    pub(crate) fn clear(&self) {
        let old = self.lock().take();
        if let Some(map) = old {
            for entry in map.into_values() {
                if let Entry::Loading(slot) = entry {
                    let (lock, ready) = &*slot;
                    *lock.lock().unwrap_or_else(|e| e.into_inner()) = Some(None);
                    ready.notify_all();
                }
            }
        }
    }
}

/// The caches of resources (CMP's `ResourceCaches`).
pub struct ResourceCaches;

impl ResourceCaches {
    /// Clears every cached resource: what is read next is loaded again (CMP's internal suspending `clear()`;
    /// [`clear_blocking`](ResourceCaches::clear_blocking) is its desktop form).
    pub async fn clear() {
        crate::strings::clear_cache();
        crate::image::clear_cache();
        crate::font::clear_cache();
        crate::plural::clear_cache();
    }

    /// The same, blocking (CMP desktop's `ResourceCaches.clearBlocking()`, `@ExperimentalResourceApi`).
    pub fn clear_blocking() {
        run_blocking(ResourceCaches::clear());
    }
}

struct Unpark(std::thread::Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// `future` run to its end on this thread (Kotlin's `runBlocking`): polled, the thread parked until it is woken.
pub fn run_blocking<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}
