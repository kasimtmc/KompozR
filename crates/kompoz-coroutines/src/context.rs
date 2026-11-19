//! What a coroutine runs with besides its code (Kotlin's CoroutineContext): elements found by their type (a
//! `MotionDurationScale`, an `InfiniteAnimationPolicy`), given to a block with [`with_context`], to a scope with
//! [`CoroutineScope::new_with_context`](crate::CoroutineScope::new_with_context), and inherited by what is launched
//! in a scope made inside (`coroutine_scope`).

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

/// Elements found by their type (Kotlin's CoroutineContext; its `Key` is the element's type here). An element is
/// any `Clone + 'static` value; one of a type already there replaces it. Clones share the elements.
#[derive(Clone, Default)]
pub struct CoroutineContext {
    elements: Rc<Vec<(TypeId, Rc<dyn Any>)>>,
}

impl CoroutineContext {
    /// No elements (Kotlin's EmptyCoroutineContext).
    pub fn new() -> CoroutineContext {
        CoroutineContext::default()
    }

    /// A context of one element.
    pub fn of<E: Clone + 'static>(element: E) -> CoroutineContext {
        CoroutineContext::new().plus_element(element)
    }

    /// This one with `element` (Kotlin's `context + element`): one of its type replaced.
    pub fn plus_element<E: Clone + 'static>(&self, element: E) -> CoroutineContext {
        let mut elements: Vec<(TypeId, Rc<dyn Any>)> =
            self.elements.iter().filter(|(t, _)| *t != TypeId::of::<E>()).cloned().collect();
        elements.push((TypeId::of::<E>(), Rc::new(element)));
        CoroutineContext { elements: Rc::new(elements) }
    }

    /// This one with `other`'s elements (Kotlin's `context + other`): `other`'s win.
    pub fn plus(&self, other: &CoroutineContext) -> CoroutineContext {
        if other.elements.is_empty() {
            return self.clone();
        }
        if self.elements.is_empty() {
            return other.clone();
        }
        let mut elements: Vec<(TypeId, Rc<dyn Any>)> =
            self.elements.iter().filter(|(t, _)| !other.elements.iter().any(|(o, _)| o == t)).cloned().collect();
        elements.extend(other.elements.iter().cloned());
        CoroutineContext { elements: Rc::new(elements) }
    }

    /// The element of type `E` (Kotlin's `context[Key]`).
    pub fn get<E: Clone + 'static>(&self) -> Option<E> {
        self.elements.iter().find(|(t, _)| *t == TypeId::of::<E>()).and_then(|(_, e)| e.downcast_ref::<E>().cloned())
    }

    /// This one without the element of type `E` (Kotlin's `minusKey`).
    pub fn minus_key<E: 'static>(&self) -> CoroutineContext {
        let elements: Vec<(TypeId, Rc<dyn Any>)> =
            self.elements.iter().filter(|(t, _)| *t != TypeId::of::<E>()).cloned().collect();
        CoroutineContext { elements: Rc::new(elements) }
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

/// The same context (Kotlin compares contexts element by element; here one context is equal to its clones, which
/// share its elements).
impl PartialEq for CoroutineContext {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.elements, &other.elements)
    }
}

impl std::ops::Add for CoroutineContext {
    type Output = CoroutineContext;
    fn add(self, other: CoroutineContext) -> CoroutineContext {
        self.plus(&other)
    }
}

impl std::fmt::Debug for CoroutineContext {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "CoroutineContext({} elements)", self.elements.len())
    }
}

thread_local! {
    /// The context of the coroutine being polled (innermost last): its job's, then a `with_context`'s inside it.
    static CONTEXT: RefCell<Vec<CoroutineContext>> = const { RefCell::new(Vec::new()) };
}

/// Pops what [`push_context`] pushed, however the poll ends.
pub(crate) struct PopContext;
impl Drop for PopContext {
    fn drop(&mut self) {
        CONTEXT.with(|c| c.borrow_mut().pop());
    }
}

/// Makes `context` the current one until the guard goes.
pub(crate) fn push_context(context: CoroutineContext) -> PopContext {
    CONTEXT.with(|c| c.borrow_mut().push(context));
    PopContext
}

/// The context of the coroutine running now (Kotlin's `coroutineContext`): its scope's, with what the
/// `with_context` blocks it is in add. Empty outside of a coroutine.
pub fn current_coroutine_context() -> CoroutineContext {
    CONTEXT.with(|c| c.borrow().last().cloned()).unwrap_or_default()
}

/// Runs `block` with `context`'s elements added to the coroutine's (Kotlin's `withContext(context) { … }`, the
/// dispatcher the same): what it reads of the context, and what it launches in a scope it makes, sees them.
pub async fn with_context<T>(context: impl Into<CoroutineContext>, block: impl Future<Output = T>) -> T {
    let context = context.into();
    WithContext { context, block: Box::pin(block) }.await
}

struct WithContext<F> {
    context: CoroutineContext,
    block: Pin<Box<F>>,
}

impl<F: Future> Future for WithContext<F> {
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let _pop = push_context(current_coroutine_context().plus(&self.context));
        self.block.as_mut().poll(cx)
    }
}
