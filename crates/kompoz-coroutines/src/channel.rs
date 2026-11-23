//! Channel: a queue between coroutines (Kotlin's Channel). One side sends, the other receives, each element once:
//! one-off UI events from a ViewModel, work handed to a consumer, a producer and its consumer kept in step.
//!
//! `send(value).await` waits while the buffer is full (a rendezvous channel, the default, until a receiver takes
//! the value); `receive().await` until there is an element. Coroutines resume on the UI thread; any thread may
//! `try_send` to a channel of `Send` elements (one of others — an `Rc`, a value an animation holds — isn't `Send`
//! itself, as Kotlin's channels carry any object: it stays on its thread). Nothing panics: `send` says whether it went through, `receive` gives `None` once the channel is
//! closed and empty (Kotlin's receiveCatching).
//!
//! A coroutine cancelled while it waits leaves the channel as it was; one cancelled after an element was handed to
//! it but before it resumed loses that element (as in Kotlin, without onUndeliveredElement).

use crate::executor::CoroutineScope;
use crate::flow::{BoxFuture, Flow, flow};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

/// What a full channel does with one more element (Kotlin's BufferOverflow).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferOverflow {
    /// `send` waits for room, `try_send` fails.
    Suspend,
    /// The oldest buffered element makes room.
    DropOldest,
    /// The new element is dropped (`send` and `try_send` still succeed).
    DropLatest,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Waiting {
    Pending,
    Done,
    Failed,
}

struct SenderSlot<T> {
    value: Option<T>,
    state: Waiting,
    waker: Option<Waker>,
}

struct ReceiverSlot<T> {
    value: Option<T>,
    state: Waiting,
    waker: Option<Waker>,
}

struct Cell<T> {
    buffer: VecDeque<T>,
    capacity: usize,
    overflow: BufferOverflow,
    closed: bool,
    senders: VecDeque<Arc<Mutex<SenderSlot<T>>>>,
    receivers: VecDeque<Arc<Mutex<ReceiverSlot<T>>>>,
}

enum Offer<T> {
    /// Handed to a receiver, buffered or dropped as the overflow says: sent.
    Sent,
    /// No room: the sender waits.
    Full(T),
    Closed,
}

type Wakeups = Vec<Waker>;

impl<T> Cell<T> {
    fn offer(&mut self, value: T, wakeups: &mut Wakeups) -> Offer<T> {
        if self.closed {
            return Offer::Closed;
        }
        while let Some(r) = self.receivers.pop_front() {
            let mut r = r.lock();
            if r.state == Waiting::Pending {
                r.value = Some(value);
                r.state = Waiting::Done;
                wakeups.extend(r.waker.take());
                return Offer::Sent;
            }
        }
        if self.buffer.len() < self.capacity {
            self.buffer.push_back(value);
            return Offer::Sent;
        }
        match self.overflow {
            BufferOverflow::Suspend => Offer::Full(value),
            BufferOverflow::DropOldest => {
                self.buffer.pop_front();
                self.buffer.push_back(value);
                Offer::Sent
            }
            BufferOverflow::DropLatest => Offer::Sent,
        }
    }

    /// The next element, a waiting sender's moved into the room it leaves (or handed straight over).
    fn take(&mut self, wakeups: &mut Wakeups) -> Option<T> {
        let mut next_sender = || {
            while let Some(s) = self.senders.pop_front() {
                if s.lock().state == Waiting::Pending {
                    return Some(s);
                }
            }
            None
        };
        if let Some(v) = self.buffer.pop_front() {
            if let Some(s) = next_sender() {
                let mut s = s.lock();
                self.buffer.push_back(s.value.take().unwrap());
                s.state = Waiting::Done;
                wakeups.extend(s.waker.take());
            }
            return Some(v);
        }
        let s = next_sender()?;
        let mut s = s.lock();
        s.state = Waiting::Done;
        wakeups.extend(s.waker.take());
        s.value.take()
    }

    fn has_waiting_sender(&self) -> bool {
        self.senders.iter().any(|s| s.lock().state == Waiting::Pending)
    }
}

fn wake(wakeups: Wakeups) {
    for w in wakeups {
        w.wake();
    }
}

/// A queue between coroutines. Clones are the same channel.
pub struct Channel<T> {
    cell: Arc<Mutex<Cell<T>>>,
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Channel { cell: self.cell.clone() }
    }
}

impl<T> PartialEq for Channel<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cell, &other.cell)
    }
}

impl<T: 'static> Default for Channel<T> {
    fn default() -> Self {
        Self::rendezvous()
    }
}

impl<T: 'static> Channel<T> {
    /// No buffer: a send waits until a receiver takes the element.
    pub const RENDEZVOUS: usize = 0;
    /// Kotlin's default buffer size.
    pub const BUFFERED: usize = 64;
    pub const UNLIMITED: usize = usize::MAX;

    /// A channel buffering `capacity` elements, doing what `overflow` says with one more.
    pub fn new(capacity: usize, overflow: BufferOverflow) -> Self {
        Channel {
            cell: Arc::new(Mutex::new(Cell {
                buffer: VecDeque::new(),
                capacity,
                overflow,
                closed: false,
                senders: VecDeque::new(),
                receivers: VecDeque::new(),
            })),
        }
    }
    pub fn rendezvous() -> Self {
        Self::new(Self::RENDEZVOUS, BufferOverflow::Suspend)
    }
    pub fn buffered() -> Self {
        Self::new(Self::BUFFERED, BufferOverflow::Suspend)
    }
    pub fn unlimited() -> Self {
        Self::new(Self::UNLIMITED, BufferOverflow::Suspend)
    }
    /// Keeps only the latest element (Kotlin's CONFLATED).
    pub fn conflated() -> Self {
        Self::new(1, BufferOverflow::DropOldest)
    }

    /// Sends `value`, waiting while there is no room; false if the channel is closed.
    pub fn send(&self, value: T) -> SendFuture<T> {
        SendFuture { cell: self.cell.clone(), value: Some(value), slot: None }
    }

    /// The next element, waiting until there is one; `None` once the channel is closed and empty.
    pub fn receive(&self) -> ReceiveFuture<T> {
        ReceiveFuture { cell: self.cell.clone(), slot: None }
    }

    /// Sends without waiting (from any thread): false if there is no room or it is closed.
    pub fn try_send(&self, value: T) -> bool {
        let mut wakeups = Vec::new();
        let sent = matches!(self.cell.lock().offer(value, &mut wakeups), Offer::Sent);
        wake(wakeups);
        sent
    }

    /// The next element if there is one now.
    pub fn try_receive(&self) -> Option<T> {
        let mut wakeups = Vec::new();
        let v = self.cell.lock().take(&mut wakeups);
        wake(wakeups);
        v
    }

    /// No more sends; what is in it is still received. False if it was closed already.
    pub fn close(&self) -> bool {
        let mut wakeups = Vec::new();
        {
            let mut c = self.cell.lock();
            if c.closed {
                return false;
            }
            c.closed = true;
            for r in std::mem::take(&mut c.receivers) {
                let mut r = r.lock();
                if r.state == Waiting::Pending {
                    r.state = Waiting::Failed;
                    wakeups.extend(r.waker.take());
                }
            }
        }
        wake(wakeups);
        true
    }

    /// Closed, and what is in it dropped: waiting senders hear false, receivers get nothing.
    pub fn cancel(&self) {
        let mut wakeups = Vec::new();
        let dropped = {
            let mut c = self.cell.lock();
            c.closed = true;
            for s in std::mem::take(&mut c.senders) {
                let mut s = s.lock();
                if s.state == Waiting::Pending {
                    s.state = Waiting::Failed;
                    wakeups.extend(s.waker.take());
                }
            }
            for r in std::mem::take(&mut c.receivers) {
                let mut r = r.lock();
                if r.state == Waiting::Pending {
                    r.state = Waiting::Failed;
                    wakeups.extend(r.waker.take());
                }
            }
            std::mem::take(&mut c.buffer)
        };
        drop(dropped);
        wake(wakeups);
    }

    /// Nothing buffered, no sender waiting.
    pub fn is_empty(&self) -> bool {
        let c = self.cell.lock();
        c.buffer.is_empty() && !c.has_waiting_sender()
    }
    pub fn is_closed_for_send(&self) -> bool {
        self.cell.lock().closed
    }
    /// Closed, and nothing left to receive.
    pub fn is_closed_for_receive(&self) -> bool {
        self.is_closed_for_send() && self.is_empty()
    }

    /// Its elements as a flow; several collectors share them, each element going to one.
    pub fn receive_as_flow(&self) -> Flow<T> {
        let ch = self.clone();
        flow(move |emit| {
            let ch = ch.clone();
            async move {
                while let Some(v) = ch.receive().await {
                    emit.emit(v).await;
                }
            }
        })
    }

    /// Calls `on_element` with each element until the channel is closed and empty; then (or if this is
    /// cancelled) the channel is cancelled: nobody consumes it any more.
    pub fn consume_each(&self, mut on_element: impl FnMut(T) + 'static) -> BoxFuture {
        let ch = self.clone();
        Box::pin(async move {
            let _cancel = CancelChannel(ch.clone());
            while let Some(v) = ch.receive().await {
                on_element(v);
            }
        })
    }
}

/// Cancels a channel when dropped.
struct CancelChannel<T: 'static>(Channel<T>);
impl<T: 'static> Drop for CancelChannel<T> {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// `channel.send(value)`: done once the value is in (true) or the channel closed (false). Dropped while it
/// waits, it leaves the channel as it was.
pub struct SendFuture<T> {
    cell: Arc<Mutex<Cell<T>>>,
    value: Option<T>,
    slot: Option<Arc<Mutex<SenderSlot<T>>>>,
}

impl<T> Unpin for SendFuture<T> {}

impl<T> Future for SendFuture<T> {
    type Output = bool;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<bool> {
        if let Some(s) = &self.slot {
            let mut g = s.lock();
            return match g.state {
                Waiting::Done => Poll::Ready(true),
                Waiting::Failed => Poll::Ready(false),
                Waiting::Pending => {
                    g.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            };
        }
        let mut wakeups = Vec::new();
        let value = self.value.take().expect("a send polled after it was done");
        let result = {
            let mut c = self.cell.lock();
            match c.offer(value, &mut wakeups) {
                Offer::Sent => Poll::Ready(true),
                Offer::Closed => Poll::Ready(false),
                Offer::Full(v) => {
                    let slot = Arc::new(Mutex::new(SenderSlot {
                        value: Some(v),
                        state: Waiting::Pending,
                        waker: Some(cx.waker().clone()),
                    }));
                    c.senders.push_back(slot.clone());
                    drop(c);
                    self.slot = Some(slot);
                    Poll::Pending
                }
            }
        };
        wake(wakeups);
        result
    }
}

impl<T> Drop for SendFuture<T> {
    fn drop(&mut self) {
        if let Some(s) = &self.slot
            && s.lock().state == Waiting::Pending
        {
            self.cell.lock().senders.retain(|x| !Arc::ptr_eq(x, s));
        }
    }
}

/// `channel.receive()`: the next element, or `None` once the channel is closed and empty. Dropped while it
/// waits, it takes nothing.
pub struct ReceiveFuture<T> {
    cell: Arc<Mutex<Cell<T>>>,
    slot: Option<Arc<Mutex<ReceiverSlot<T>>>>,
}

impl<T> Unpin for ReceiveFuture<T> {}

impl<T> Future for ReceiveFuture<T> {
    type Output = Option<T>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        if let Some(s) = &self.slot {
            let mut g = s.lock();
            return match g.state {
                Waiting::Done => Poll::Ready(g.value.take()),
                Waiting::Failed => Poll::Ready(None),
                Waiting::Pending => {
                    g.waker = Some(cx.waker().clone());
                    Poll::Pending
                }
            };
        }
        let mut wakeups = Vec::new();
        let result = {
            let mut c = self.cell.lock();
            match c.take(&mut wakeups) {
                Some(v) => Poll::Ready(Some(v)),
                None if c.closed => Poll::Ready(None),
                None => {
                    let slot = Arc::new(Mutex::new(ReceiverSlot {
                        value: None,
                        state: Waiting::Pending,
                        waker: Some(cx.waker().clone()),
                    }));
                    c.receivers.push_back(slot.clone());
                    drop(c);
                    self.slot = Some(slot);
                    Poll::Pending
                }
            }
        };
        wake(wakeups);
        result
    }
}

impl<T> Drop for ReceiveFuture<T> {
    fn drop(&mut self) {
        if let Some(s) = &self.slot
            && s.lock().state == Waiting::Pending
        {
            self.cell.lock().receivers.retain(|x| !Arc::ptr_eq(x, s));
        }
    }
}

/// A channel `producer` sends to from a coroutine in `scope` (Kotlin's produce): closed when it is done,
/// cancelled if it is.
pub fn produce<T: 'static, Fut: Future<Output = ()> + 'static>(
    scope: &CoroutineScope,
    capacity: usize,
    producer: impl FnOnce(Channel<T>) -> Fut,
) -> Channel<T> {
    let channel = Channel::new(capacity, BufferOverflow::Suspend);
    let fut = producer(channel.clone());
    struct Finish<T: 'static>(Channel<T>, bool);
    impl<T: 'static> Drop for Finish<T> {
        fn drop(&mut self) {
            if self.1 {
                self.0.close();
            } else {
                self.0.cancel();
            }
        }
    }
    // (made outside the coroutine: cancelled before it was dispatched, the channel is cancelled too)
    let finish = Finish(channel.clone(), false);
    scope.launch(async move {
        let mut finish = finish;
        fut.await;
        finish.1 = true;
    });
    channel
}
