//! What infinite animations may do (Compose's InfiniteAnimationPolicy): a coroutine context element.

use kompoz_coroutines::CoroutineContext;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// A block an [`InfiniteAnimationPolicy`] is given: one step of an infinite animation, its value kept aside.
pub type InfiniteOperation = Pin<Box<dyn Future<Output = ()>>>;

/// Decides how infinite animations run (Compose's InfiniteAnimationPolicy, a coroutine context element): each
/// frame of one goes through [`on_infinite_operation`](InfiniteAnimationPolicy::on_infinite_operation), which may
/// run it, trace it, or not run it (a test's policy cancels them while its clock advances on its own: Kotlin's
/// `throw CancellationException`, [`cancel_current`](kompoz_coroutines::cancel_current) here). Without one in the
/// context they just run. Clones are the same policy.
#[derive(Clone)]
pub struct InfiniteAnimationPolicy {
    policy: Rc<dyn Fn(InfiniteOperation) -> InfiniteOperation>,
}

impl InfiniteAnimationPolicy {
    /// A policy: given the block, the future that runs it (or doesn't).
    pub fn new(policy: impl Fn(InfiniteOperation) -> InfiniteOperation + 'static) -> InfiniteAnimationPolicy {
        InfiniteAnimationPolicy { policy: Rc::new(policy) }
    }

    /// Applies the policy to `block`, part of an infinite animation (Compose's `onInfiniteOperation`): its value
    /// if the policy ran it. A policy that returns without running it (nor cancelling the coroutine) is wrong:
    /// this panics then, as Kotlin's has to return the block's value.
    pub async fn on_infinite_operation<R: 'static>(&self, block: impl Future<Output = R> + 'static) -> R {
        let out: Rc<RefCell<Option<R>>> = Rc::new(RefCell::new(None));
        let slot = out.clone();
        (self.policy)(Box::pin(async move {
            let value = block.await;
            *slot.borrow_mut() = Some(value);
        }))
        .await;
        let value = out.borrow_mut().take();
        value.expect("kompoz: an InfiniteAnimationPolicy returned without running its block")
    }
}

impl std::fmt::Debug for InfiniteAnimationPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("InfiniteAnimationPolicy")
    }
}

/// As Kotlin's element is a context of its own: `with_context(policy, block)`.
impl From<InfiniteAnimationPolicy> for CoroutineContext {
    fn from(policy: InfiniteAnimationPolicy) -> CoroutineContext {
        CoroutineContext::of(policy)
    }
}
