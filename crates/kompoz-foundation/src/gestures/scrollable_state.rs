//! What scrolls, as Compose's ScrollableState.kt and ScrollExtensions.kt: a [`ScrollableState`] takes scroll
//! deltas inside a mutation ([`ScrollableState::scroll`], one at a time, a newer one of at least its priority
//! cancelling an older), in its [`ScrollScope`]; [`ScrollableState()`] makes one of a closure consuming them.

use kompoz_animation::{AnimationSpec, MutatePriority, MutatorMutex, animate, spring};
use kompoz_runtime::{MutableState, kompozable, mutable_state_of, named, named_args, remember, remember_updated_state};
use std::any::Any;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

/// A future a scrollable's mutation, fling or overscroll runs in (Kotlin's suspending call).
pub type LocalFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// What a scroll mutation scrolls with (Compose's ScrollScope): shared with the block it is given.
pub trait ScrollScope {
    /// Scrolls forward by `pixels` (back if negative): how far it went (Compose's `scrollBy`).
    fn scroll_by(&self, pixels: f32) -> f32;
}

/// A block scrolling in a [`ScrollScope`] (Compose's `suspend ScrollScope.() -> Unit`).
pub type ScrollBlock = Box<dyn FnOnce(Rc<dyn ScrollScope>) -> LocalFuture<'static, ()>>;

/// A state that scrolls (Compose's ScrollableState): `scroll` runs a block taking deltas in its [`ScrollScope`],
/// one block at a time (Compose's MutatorMutex: a newer one cancels an older one of its priority or lower);
/// `dispatch_raw_delta` takes a delta outside of that (nested scrolling's); whether it scrolls now and can scroll
/// on. A state is a cheap handle: clones are the same state, and two are equal when they are (what scroll
/// modifiers compare it by). The generic ways of calling it are [`ScrollableStateExtensions`]'s.
pub trait ScrollableState: Any + ScrollableStateEq {
    /// Runs `block` as the mutation of `scroll_priority` (Compose's `scroll`): the one running is cancelled if its
    /// priority isn't higher, else this one is (its coroutine cancelled). Through
    /// [`ScrollableStateExtensions::scroll`] with a closure.
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()>;

    /// Takes `delta` px at once, outside of any mutation, nested scrolling and overscroll left out: what it took
    /// (Compose's `dispatchRawDelta`).
    fn dispatch_raw_delta(&self, delta: f32) -> f32;

    /// Whether a gesture, a fling or a program scrolls it now (Compose's `isScrollInProgress`).
    fn is_scroll_in_progress(&self) -> bool;

    /// Whether it can scroll forward: take a positive delta (Compose's `canScrollForward`; true unless it says).
    fn can_scroll_forward(&self) -> bool {
        true
    }

    /// Whether it can scroll back (Compose's `canScrollBackward`).
    fn can_scroll_backward(&self) -> bool {
        true
    }

    /// Whether it scrolls forward now, or did in its last scroll (Compose's `lastScrolledForward`).
    fn last_scrolled_forward(&self) -> bool {
        false
    }

    /// Whether it scrolls back now, or did in its last scroll (Compose's `lastScrolledBackward`).
    fn last_scrolled_backward(&self) -> bool {
        false
    }

    /// What a scroll indicator (a scrollbar) shows of it, if it can tell (Compose's `scrollIndicatorState`).
    fn scroll_indicator_state(&self) -> Option<Rc<dyn ScrollIndicatorState>> {
        None
    }
}

/// How scrollable states compare (a blanket impl for every `PartialEq` one): by what they are.
#[doc(hidden)]
pub trait ScrollableStateEq {
    fn scrollable_state_eq(&self, other: &dyn Any) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T: PartialEq + Any> ScrollableStateEq for T {
    fn scrollable_state_eq(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|o| self == o)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PartialEq for dyn ScrollableState {
    fn eq(&self, other: &dyn ScrollableState) -> bool {
        self.scrollable_state_eq(other.as_any())
    }
}

impl fmt::Debug for dyn ScrollableState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ScrollableState")
    }
}

/// The calls of any [`ScrollableState`] (Compose's `scroll` with a lambda, and ScrollExtensions.kt's
/// `scrollBy`, `animateScrollBy`, `stopScroll`); a `dyn ScrollableState` has them as its own methods.
#[named(trait ScrollableStateExtensions)]
impl<S: ScrollableState> S {
    /// Runs `block` as the mutation of `scroll_priority` (Compose's `scroll { … }`): see
    /// [`ScrollableState::scroll_block`].
    fn scroll<F, Fut>(
        &self,
        #[default(MutatePriority::Default)] scroll_priority: MutatePriority,
        block: F,
    ) -> impl Future<Output = ()>
    where
        F: FnOnce(Rc<dyn ScrollScope>) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        scroll_of(self, scroll_priority, block)
    }

    /// Scrolls by `value` px at once, in a mutation of its own: how far it went (Compose's `scrollBy`).
    fn scroll_by(&self, value: f32) -> impl Future<Output = f32> {
        scroll_by_of(self, value)
    }

    /// Scrolls by `value` px along `animation_spec` (a spring by default), in a mutation of its own: how far it
    /// went (Compose's `animateScrollBy`).
    fn animate_scroll_by(
        &self,
        value: f32,
        #[default(Arc::new(spring::<f32>()))] animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> impl Future<Output = f32> {
        animate_scroll_by_of(self, value, animation_spec)
    }

    /// Stops what scrolls it now (an animation, a fling) unless its priority is higher than `scroll_priority`,
    /// by taking the mutation with nothing to do (Compose's `stopScroll`).
    fn stop_scroll(
        &self,
        #[default(MutatePriority::Default)] scroll_priority: MutatePriority,
    ) -> impl Future<Output = ()> {
        self.scroll_block(scroll_priority, Box::new(|_| Box::pin(async {})))
    }
}

/// A state shared as any kind of state (what a `ScrollableState` value is in Kotlin): its state's calls, so
/// [`ScrollableStateExtensions`]' are its too.
impl ScrollableState for Rc<dyn ScrollableState> {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        (**self).scroll_block(scroll_priority, block)
    }
    fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        (**self).dispatch_raw_delta(delta)
    }
    fn is_scroll_in_progress(&self) -> bool {
        (**self).is_scroll_in_progress()
    }
    fn can_scroll_forward(&self) -> bool {
        (**self).can_scroll_forward()
    }
    fn can_scroll_backward(&self) -> bool {
        (**self).can_scroll_backward()
    }
    fn last_scrolled_forward(&self) -> bool {
        (**self).last_scrolled_forward()
    }
    fn last_scrolled_backward(&self) -> bool {
        (**self).last_scrolled_backward()
    }
    fn scroll_indicator_state(&self) -> Option<Rc<dyn ScrollIndicatorState>> {
        (**self).scroll_indicator_state()
    }
}

fn scroll_of<'a, S, F, Fut>(state: &'a S, scroll_priority: MutatePriority, block: F) -> LocalFuture<'a, ()>
where
    S: ScrollableState + ?Sized,
    F: FnOnce(Rc<dyn ScrollScope>) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    state.scroll_block(scroll_priority, Box::new(move |scope| Box::pin(block(scope))))
}

/// Compose's `ScrollableState.scrollBy`.
pub(crate) fn scroll_by_of<S: ScrollableState + ?Sized>(state: &S, value: f32) -> impl Future<Output = f32> {
    let consumed = Rc::new(std::cell::Cell::new(0.0f32));
    let c = consumed.clone();
    let scroll = state.scroll_block(
        MutatePriority::Default,
        Box::new(move |scope| {
            Box::pin(async move {
                c.set(scope.scroll_by(value));
            })
        }),
    );
    async move {
        scroll.await;
        consumed.get()
    }
}

/// Compose's `ScrollableState.animateScrollBy`.
pub(crate) fn animate_scroll_by_of<S: ScrollableState + ?Sized>(
    state: &S,
    value: f32,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
) -> impl Future<Output = f32> {
    let previous_value = Rc::new(std::cell::Cell::new(0.0f32));
    let p = previous_value.clone();
    let scroll = state.scroll_block(
        MutatePriority::Default,
        Box::new(move |scope| {
            Box::pin(async move {
                let p = p.clone();
                named_args!(animate(
                    0.0,
                    value,
                    animation_spec = animation_spec,
                    block = {
                        move |current_value: f32, _| {
                            p.set(p.get() + scope.scroll_by(current_value - p.get()));
                        }
                    }
                ))
                .await;
            })
        }),
    );
    async move {
        scroll.await;
        previous_value.get()
    }
}

/// What a scroll indicator shows of a scrollable (Compose's ScrollIndicatorState): how far it scrolled, how long
/// its content is and how much of it shows, px.
pub trait ScrollIndicatorState {
    /// How far its content is scrolled from its start (Compose's `scrollOffset`).
    fn scroll_offset(&self) -> i32;
    /// Its content's whole length along its axis (Compose's `contentSize`).
    fn content_size(&self) -> i32;
    /// The length that shows of it (Compose's `viewportSize`).
    fn viewport_size(&self) -> i32;
}

// ---------------------------------------------------------------------------------------------------------------
// The default state

/// A scrollable state of a closure that takes deltas (Compose's DefaultScrollableState, made by
/// [`ScrollableState()`]): its scope's `scroll_by` gives the closure the delta (nothing for NaN) and notes which way
/// it went.
#[derive(Clone)]
pub struct DefaultScrollableState {
    inner: Rc<DefaultInner>,
}

struct DefaultInner {
    on_delta: Rc<dyn Fn(f32) -> f32>,
    scroll_mutex: MutatorMutex,
    is_scrolling_state: MutableState<bool>,
    is_last_scroll_forward_state: MutableState<bool>,
    is_last_scroll_backward_state: MutableState<bool>,
}

impl PartialEq for DefaultScrollableState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for DefaultScrollableState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DefaultScrollableState({:p})", Rc::as_ptr(&self.inner))
    }
}

/// The scope a [`DefaultScrollableState`] scrolls in.
struct DefaultScrollScope(Rc<DefaultInner>);

impl ScrollScope for DefaultScrollScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        if pixels.is_nan() {
            return 0.0;
        }
        let delta = (self.0.on_delta)(pixels);
        self.0.is_last_scroll_forward_state.set(delta > 0.0);
        self.0.is_last_scroll_backward_state.set(delta < 0.0);
        delta
    }
}

/// Puts `is_scrolling` back when the mutation ends however (Compose's `finally`).
struct ScrollingFlag(MutableState<bool>);

impl Drop for ScrollingFlag {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl DefaultScrollableState {
    fn new(on_delta: Rc<dyn Fn(f32) -> f32>) -> DefaultScrollableState {
        DefaultScrollableState {
            inner: Rc::new(DefaultInner {
                on_delta,
                scroll_mutex: MutatorMutex::new(),
                is_scrolling_state: mutable_state_of(false),
                is_last_scroll_forward_state: mutable_state_of(false),
                is_last_scroll_backward_state: mutable_state_of(false),
            }),
        }
    }
}

impl ScrollableState for DefaultScrollableState {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: ScrollBlock) -> LocalFuture<'_, ()> {
        let inner = self.inner.clone();
        let scope: Rc<dyn ScrollScope> = Rc::new(DefaultScrollScope(inner.clone()));
        // (Compose's coroutineScope around mutateWith: the mutex's own)
        let mutation = named_args!(inner.scroll_mutex.mutate_with(scope, scroll_priority, {
            let inner = inner.clone();
            move |scope: Rc<dyn ScrollScope>| async move {
                inner.is_scrolling_state.set(true);
                let _flag = ScrollingFlag(inner.is_scrolling_state.clone());
                block(scope).await;
            }
        }));
        Box::pin(mutation)
    }

    fn dispatch_raw_delta(&self, delta: f32) -> f32 {
        (self.inner.on_delta)(delta)
    }

    fn is_scroll_in_progress(&self) -> bool {
        self.inner.is_scrolling_state.get()
    }

    fn last_scrolled_forward(&self) -> bool {
        self.inner.is_last_scroll_forward_state.get()
    }

    fn last_scrolled_backward(&self) -> bool {
        self.inner.is_last_scroll_backward_state.get()
    }
}

/// A scrollable state of `consume_scroll_delta` (Compose's `ScrollableState(consumeScrollDelta)`): given each
/// delta, px (positive forward), it scrolls what it can and gives back how much of it it took — what it doesn't
/// take goes on to the nested scroll parents and the overscroll.
pub fn ScrollableState(consume_scroll_delta: impl Fn(f32) -> f32 + 'static) -> DefaultScrollableState {
    DefaultScrollableState::new(Rc::new(consume_scroll_delta))
}

/// A [`ScrollableState()`] remembered where it is called, calling the latest `consume_scroll_delta` (Compose's
/// `rememberScrollableState`).
#[kompozable]
pub fn remember_scrollable_state(consume_scroll_delta: impl Fn(f32) -> f32 + 'static) -> DefaultScrollableState {
    let lambda_state = remember_updated_state!(Rc::new(consume_scroll_delta) as Rc<dyn Fn(f32) -> f32>);
    remember(move || ScrollableState(move |delta| (lambda_state.get())(delta)))
}
