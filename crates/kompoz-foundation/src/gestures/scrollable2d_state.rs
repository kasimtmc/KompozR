//! What scrolls both ways, as Compose's Scrollable2DState.kt and ScrollExtensions.kt's 2D ones: a
//! [`Scrollable2DState`] takes 2D scroll deltas inside a mutation ([`Scrollable2DState::scroll_block`], one at a
//! time, a newer one of at least its priority cancelling an older), in its [`Scroll2DScope`];
//! [`Scrollable2DState()`] makes one of a closure consuming them.

use super::scrollable_state::{LocalFuture, ScrollableStateEq};
use kompoz_animation::{AnimationSpec, MutatePriority, MutatorMutex, VectorConverter, animate_value, spring};
use kompoz_graphics::Offset;
use kompoz_runtime::{MutableState, kompozable, mutable_state_of, named, named_args, remember, remember_updated_state};
use std::cell::Cell;
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

/// What a 2D scroll mutation scrolls with (Compose's Scroll2DScope).
pub trait Scroll2DScope {
    /// Scrolls by `delta` px: how far it went (Compose's `scrollBy`).
    fn scroll_by(&self, delta: Offset) -> Offset;
}

/// A block scrolling in a [`Scroll2DScope`] (Compose's `suspend Scroll2DScope.() -> Unit`).
pub type Scroll2DBlock = Box<dyn FnOnce(Rc<dyn Scroll2DScope>) -> LocalFuture<'static, ()>>;

/// A state that scrolls both ways (Compose's Scrollable2DState): `scroll_block` runs a block taking deltas in its
/// [`Scroll2DScope`], one block at a time; `dispatch_raw_delta` takes a delta outside of that (nested
/// scrolling's); whether it scrolls now, and whether it can scroll by an offset (its direction). A cheap handle,
/// equal to its clones (what `scrollable2d` compares it by). The generic ways of calling it are
/// [`Scrollable2DStateExtensions`]'s.
pub trait Scrollable2DState: ScrollableStateEq + 'static {
    /// Runs `block` as the mutation of `scroll_priority` (Compose's `scroll`). Through
    /// [`Scrollable2DStateExtensions::scroll`] with a closure.
    fn scroll_block(&self, scroll_priority: MutatePriority, block: Scroll2DBlock) -> LocalFuture<'_, ()>;

    /// Takes `delta` px at once, outside of any mutation, nested scrolling and overscroll left out: what it took
    /// (Compose's `dispatchRawDelta`).
    fn dispatch_raw_delta(&self, delta: Offset) -> Offset;

    /// Whether a gesture, a fling or a program scrolls it now (Compose's `isScrollInProgress`).
    fn is_scroll_in_progress(&self) -> bool;

    /// Whether it has room to scroll the way `offset` points (Compose's `canScroll`).
    fn can_scroll(&self, offset: Offset) -> bool;
}

impl PartialEq for dyn Scrollable2DState {
    fn eq(&self, other: &dyn Scrollable2DState) -> bool {
        self.scrollable_state_eq(ScrollableStateEq::as_any(other))
    }
}

impl fmt::Debug for dyn Scrollable2DState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Scrollable2DState")
    }
}

/// A state shared as any kind of 2D state: its state's calls.
impl Scrollable2DState for Rc<dyn Scrollable2DState> {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: Scroll2DBlock) -> LocalFuture<'_, ()> {
        (**self).scroll_block(scroll_priority, block)
    }
    fn dispatch_raw_delta(&self, delta: Offset) -> Offset {
        (**self).dispatch_raw_delta(delta)
    }
    fn is_scroll_in_progress(&self) -> bool {
        (**self).is_scroll_in_progress()
    }
    fn can_scroll(&self, offset: Offset) -> bool {
        (**self).can_scroll(offset)
    }
}

/// The calls of any [`Scrollable2DState`] (Compose's `scroll` with a lambda, and ScrollExtensions.kt's
/// `scrollBy`, `animateScrollBy`, `stopScroll` of a 2D state).
#[named(trait Scrollable2DStateExtensions)]
impl<S: Scrollable2DState> S {
    /// Runs `block` as the mutation of `scroll_priority` (Compose's `scroll { … }`).
    fn scroll<F, Fut>(
        &self,
        #[default(MutatePriority::Default)] scroll_priority: MutatePriority,
        block: F,
    ) -> impl Future<Output = ()>
    where
        F: FnOnce(Rc<dyn Scroll2DScope>) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.scroll_block(scroll_priority, Box::new(move |scope| Box::pin(block(scope))))
    }

    /// Scrolls by `value` px at once, in a mutation of its own: how far it went (Compose's `scrollBy`).
    fn scroll_by(&self, value: Offset) -> impl Future<Output = Offset> {
        let consumed = Rc::new(Cell::new(Offset::Zero));
        let c = consumed.clone();
        let scroll = self.scroll_block(
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

    /// Scrolls by `value` px along `animation_spec` (a spring by default), in a mutation of its own: how far it
    /// went (Compose's `animateScrollBy`).
    fn animate_scroll_by(
        &self,
        value: Offset,
        #[default(Arc::new(spring::<Offset>()))] animation_spec: Arc<dyn AnimationSpec<Offset>>,
    ) -> impl Future<Output = Offset> {
        let previous_value = Rc::new(Cell::new(Offset::Zero));
        let p = previous_value.clone();
        let scroll = self.scroll_block(
            MutatePriority::Default,
            Box::new(move |scope| {
                Box::pin(async move {
                    let p = p.clone();
                    named_args!(animate_value(
                        Offset::VectorConverter,
                        Offset::Zero,
                        value,
                        animation_spec = animation_spec,
                        block = {
                            move |current_value: Offset, _| {
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

    /// Stops what scrolls it now unless its priority is higher than `scroll_priority`, by taking the mutation
    /// with nothing to do (Compose's `stopScroll`).
    fn stop_scroll(
        &self,
        #[default(MutatePriority::Default)] scroll_priority: MutatePriority,
    ) -> impl Future<Output = ()> {
        self.scroll_block(scroll_priority, Box::new(|_| Box::pin(async {})))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The default state

/// A 2D scrollable state of a closure that takes deltas (Compose's DefaultScrollable2DState, made by
/// [`Scrollable2DState()`]): its scope gives the closure each delta (nothing for NaN); it can always scroll.
#[derive(Clone)]
pub struct DefaultScrollable2DState {
    inner: Rc<DefaultInner>,
}

struct DefaultInner {
    on_delta: Rc<dyn Fn(Offset) -> Offset>,
    scroll_mutex: MutatorMutex,
    is_scrolling_state: MutableState<bool>,
}

impl PartialEq for DefaultScrollable2DState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for DefaultScrollable2DState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DefaultScrollable2DState({:p})", Rc::as_ptr(&self.inner))
    }
}

struct DefaultScroll2DScope(Rc<dyn Fn(Offset) -> Offset>);

impl Scroll2DScope for DefaultScroll2DScope {
    fn scroll_by(&self, delta: Offset) -> Offset {
        if delta.x.is_nan() || delta.y.is_nan() {
            return Offset::Zero;
        }
        (self.0)(delta)
    }
}

/// Puts `is_scrolling` back when the mutation ends however (Compose's `finally`).
struct ScrollingFlag(MutableState<bool>);

impl Drop for ScrollingFlag {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

impl Scrollable2DState for DefaultScrollable2DState {
    fn scroll_block(&self, scroll_priority: MutatePriority, block: Scroll2DBlock) -> LocalFuture<'_, ()> {
        let inner = self.inner.clone();
        let scope: Rc<dyn Scroll2DScope> = Rc::new(DefaultScroll2DScope(inner.on_delta.clone()));
        let mutation = named_args!(inner.scroll_mutex.mutate_with(scope, scroll_priority, {
            let inner = inner.clone();
            move |scope: Rc<dyn Scroll2DScope>| async move {
                inner.is_scrolling_state.set(true);
                let _flag = ScrollingFlag(inner.is_scrolling_state.clone());
                block(scope).await;
            }
        }));
        Box::pin(mutation)
    }

    fn dispatch_raw_delta(&self, delta: Offset) -> Offset {
        (self.inner.on_delta)(delta)
    }

    fn is_scroll_in_progress(&self) -> bool {
        self.inner.is_scrolling_state.get()
    }

    fn can_scroll(&self, _offset: Offset) -> bool {
        true
    }
}

/// A 2D scrollable state of `consume_scroll_delta` (Compose's `Scrollable2DState(consumeScrollDelta)`): given each
/// delta, px, it scrolls what it can and gives back how much of it it took — what it doesn't take goes on to the
/// nested scroll parents and the overscroll.
pub fn Scrollable2DState(consume_scroll_delta: impl Fn(Offset) -> Offset + 'static) -> DefaultScrollable2DState {
    DefaultScrollable2DState {
        inner: Rc::new(DefaultInner {
            on_delta: Rc::new(consume_scroll_delta),
            scroll_mutex: MutatorMutex::new(),
            is_scrolling_state: mutable_state_of(false),
        }),
    }
}

/// A [`Scrollable2DState()`] remembered where it is called, calling the latest `consume_scroll_delta` (Compose's
/// `rememberScrollable2DState`).
#[kompozable]
pub fn remember_scrollable2d_state(
    consume_scroll_delta: impl Fn(Offset) -> Offset + 'static,
) -> DefaultScrollable2DState {
    let lambda_state = remember_updated_state::<Rc<dyn Fn(Offset) -> Offset>>(Rc::new(consume_scroll_delta));
    remember(move || Scrollable2DState(move |delta| (lambda_state.get())(delta)))
}
