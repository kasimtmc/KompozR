//! Work off the UI thread: `with_background(|| …).await` (Compose's `withContext(Dispatchers.IO / Default)`).

use parking_lot::{Condvar, Mutex};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Waker};

static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);

type Work = Box<dyn FnOnce() + Send>;

/// A few workers taking work in turn; never torn down (they may still be busy as the app ends).
struct Pool {
    queue: Mutex<VecDeque<Work>>,
    ready: Condvar,
}

fn pool() -> &'static Pool {
    static POOL: OnceLock<&'static Pool> = OnceLock::new();
    POOL.get_or_init(|| {
        let pool: &'static Pool =
            Box::leak(Box::new(Pool { queue: Mutex::new(VecDeque::new()), ready: Condvar::new() }));
        let n = std::thread::available_parallelism().map_or(2, |n| n.get() / 2).clamp(2, 4);
        for i in 0..n {
            std::thread::Builder::new()
                .name(format!("kompoz-background-{i}"))
                .spawn(move || {
                    loop {
                        let work = {
                            let mut q = pool.queue.lock();
                            while q.is_empty() {
                                pool.ready.wait(&mut q);
                            }
                            q.pop_front().unwrap()
                        };
                        work();
                        IN_FLIGHT.fetch_sub(1, Ordering::AcqRel); // after it handed its result over
                    }
                })
                .expect("kompoz: a background worker");
        }
        pool
    })
}

/// Runs `work` on one of a few worker threads.
pub fn run_in_background(work: impl FnOnce() + Send + 'static) {
    IN_FLIGHT.fetch_add(1, Ordering::AcqRel);
    let p = pool();
    p.queue.lock().push_back(Box::new(work));
    p.ready.notify_one();
}

/// How much background work is running or waiting (what a test waits for).
pub fn background_work_in_flight() -> usize {
    IN_FLIGHT.load(Ordering::Acquire)
}

struct Shared<T> {
    result: Option<std::thread::Result<T>>,
    waker: Option<Waker>,
}

/// `with_background(|| load()).await`: `work` runs on a worker thread (files, decoding, heavy sums), the coroutine
/// carrying on on the UI thread with what it gave; a panic in it panics again here. Cancelled meanwhile, the work
/// still runs to its end and what it gives is dropped. It may write states, and reads them in a snapshot.
pub fn with_background<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Background<T> {
    let shared = Arc::new(Mutex::new(Shared { result: None, waker: None }));
    let s = shared.clone();
    run_in_background(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
        let waker = {
            let mut g = s.lock();
            g.result = Some(r);
            g.waker.take()
        };
        if let Some(w) = waker {
            w.wake();
        }
    });
    Background { shared }
}

pub struct Background<T> {
    shared: Arc<Mutex<Shared<T>>>,
}

impl<T> Future for Background<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let mut g = self.shared.lock();
        match g.result.take() {
            Some(Ok(v)) => Poll::Ready(v),
            Some(Err(panic)) => {
                drop(g);
                std::panic::resume_unwind(panic)
            }
            None => {
                g.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}
