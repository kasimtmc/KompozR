//! Structured concurrency inside a coroutine: `coroutine_scope`, `supervisor_scope`, `with_timeout`.

use crate::context::current_coroutine_context;
use crate::executor::{CoroutineScope, Failure, ScopeState, cancel_current, current};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::task::Poll;
use std::time::Duration;

/// Cancels a scope when dropped: the awaiting coroutine cancelled takes the scope's children with it.
struct CancelOnDrop(Option<Rc<ScopeState>>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(s) = self.0.take() {
            s.cancel(None);
        }
    }
}

enum Outcome<T> {
    Value(T),
    Failed(Failure),
    TimedOut,
    /// The block was cancelled (its scope cancelled through the handle it was given, or the block's job): as Kotlin's
    /// CancellationException leaving `coroutineScope`, the awaiting coroutine is cancelled too.
    Cancelled,
}

/// Runs `block` as a child of a scope of its own, with what it launches there; done once all of them are.
async fn run_scope<T: 'static, Fut: Future<Output = T> + 'static>(
    block: impl FnOnce(CoroutineScope) -> Fut,
    supervisor: bool,
    timeout: Option<Duration>,
) -> Outcome<T> {
    let (d, job) = current();
    let scope = ScopeState::new(&d, true, supervisor, current_coroutine_context());
    let mut guard = CancelOnDrop(Some(scope.clone()));
    let timed_out = Rc::new(Cell::new(false));
    if let Some(t) = timeout {
        let (s, flag) = (Rc::downgrade(&scope), timed_out.clone());
        let when = d.now().saturating_add(t.as_nanos().min(i64::MAX as u128) as i64);
        d.schedule_call(
            when,
            Box::new(move || {
                if let Some(s) = s.upgrade()
                    && !s.is_empty()
                {
                    flag.set(true);
                    s.cancel(None);
                }
            }),
            &job,
        );
    }
    let value: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
    let slot = value.clone();
    let body = block(CoroutineScope(scope.clone()));
    scope.launch(
        Box::pin(async move {
            let v = body.await;
            *slot.borrow_mut() = Some(v);
        }),
        false,
        true,
    );
    std::future::poll_fn(|cx| {
        if scope.is_empty() {
            Poll::Ready(())
        } else {
            scope.set_when_empty(cx.waker().clone());
            Poll::Pending
        }
    })
    .await;
    guard.0 = None;
    if let Some(f) = scope.take_failure() {
        return Outcome::Failed(f);
    }
    match value.borrow_mut().take() {
        Some(v) if !timed_out.get() => Outcome::Value(v),
        _ if timed_out.get() => Outcome::TimedOut,
        _ => Outcome::Cancelled,
    }
}

/// `coroutine_scope(|scope| async move { scope.launch(…); … }).await` (Kotlin's coroutineScope): done once the
/// block and everything it launched in `scope` are, with the block's value. A child failing cancels the others,
/// and the failure goes on where this is awaited.
pub async fn coroutine_scope<T: 'static, Fut: Future<Output = T> + 'static>(
    block: impl FnOnce(CoroutineScope) -> Fut,
) -> T {
    match run_scope(block, false, None).await {
        Outcome::Value(v) => v,
        Outcome::Failed(f) => f.resume(),
        Outcome::TimedOut => unreachable!("cancelled without a timeout"),
        Outcome::Cancelled => cancel_current().await,
    }
}

/// The same, its children failing on their own (Kotlin's supervisorScope): a child's failure is reported and
/// cancels nothing; only the block's own fails the scope.
pub async fn supervisor_scope<T: 'static, Fut: Future<Output = T> + 'static>(
    block: impl FnOnce(CoroutineScope) -> Fut,
) -> T {
    match run_scope(block, true, None).await {
        Outcome::Value(v) => v,
        Outcome::Failed(f) => f.resume(),
        Outcome::TimedOut => unreachable!("cancelled without a timeout"),
        Outcome::Cancelled => cancel_current().await,
    }
}

/// Runs `block` as `coroutine_scope` does, catching its own cancellation (Kotlin's `try { … } catch (e:
/// CancellationException)` around suspending code): `Err(Cancelled)` when the block was cancelled from inside —
/// a non-suspending call under it "threw" by cancelling [`current_job`](crate::current_job), as a fling's
/// `scrollBy` throws when its node left — while the coroutine awaiting this goes on. Cancelled from outside, the
/// awaiting coroutine is dropped with it, as a Kotlin catch would rethrow at its next suspension.
pub async fn catch_cancellation<T: 'static, Fut: Future<Output = T> + 'static>(
    block: impl FnOnce(CoroutineScope) -> Fut,
) -> Result<T, crate::Cancelled> {
    match run_scope(block, false, None).await {
        Outcome::Value(v) => Ok(v),
        Outcome::Failed(f) => f.resume(),
        Outcome::TimedOut => unreachable!("cancelled without a timeout"),
        Outcome::Cancelled => Err(crate::Cancelled),
    }
}

/// The time given to [`with_timeout`] ran out (Kotlin's TimeoutCancellationException).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeoutCancellation;

impl std::fmt::Display for TimeoutCancellation {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("timed out")
    }
}
impl std::error::Error for TimeoutCancellation {}

/// `coroutine_scope` with a time limit (Kotlin's withTimeout): when it runs out, all of it is cancelled and this
/// gives `Err(TimeoutCancellation)`.
pub async fn with_timeout<T: 'static, Fut: Future<Output = T> + 'static>(
    limit: Duration,
    block: impl FnOnce(CoroutineScope) -> Fut,
) -> Result<T, TimeoutCancellation> {
    match run_scope(block, false, Some(limit)).await {
        Outcome::Value(v) => Ok(v),
        Outcome::Failed(f) => f.resume(),
        Outcome::TimedOut => Err(TimeoutCancellation),
        Outcome::Cancelled => cancel_current().await,
    }
}

/// The same, giving `None` when the time runs out (Kotlin's withTimeoutOrNull).
pub async fn with_timeout_or_null<T: 'static, Fut: Future<Output = T> + 'static>(
    limit: Duration,
    block: impl FnOnce(CoroutineScope) -> Fut,
) -> Option<T> {
    with_timeout(limit, block).await.ok()
}
