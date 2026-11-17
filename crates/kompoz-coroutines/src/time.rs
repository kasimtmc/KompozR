//! Waiting in a coroutine: for a time on the dispatcher's clock, or for the next frame.

use crate::executor::current;
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

/// `delay(duration).await`: waits that long on the dispatcher's clock (the frames' time): a `Duration`, or
/// milliseconds as Kotlin's `delay(timeMillis: Long)` takes them (`delay(300)`; none at all for zero or less).
pub fn delay(duration: impl DelayTime) -> Delay {
    Delay { nanos: duration.nanos(), fired: None }
}

/// How long [`delay`] waits: a `Duration`, or a number of milliseconds (Kotlin's `Long`, an `Int` as Kotlin widens
/// it).
pub trait DelayTime {
    /// In nanoseconds, within `i64` (zero or less: no wait).
    fn nanos(self) -> i64;
}
impl DelayTime for Duration {
    fn nanos(self) -> i64 {
        self.as_nanos().min(i64::MAX as u128) as i64
    }
}
impl DelayTime for i64 {
    fn nanos(self) -> i64 {
        self.saturating_mul(1_000_000)
    }
}
impl DelayTime for i32 {
    fn nanos(self) -> i64 {
        (self as i64).saturating_mul(1_000_000)
    }
}
impl DelayTime for u64 {
    fn nanos(self) -> i64 {
        (self.min(i64::MAX as u64) as i64).saturating_mul(1_000_000)
    }
}

/// A delay of `ms` milliseconds.
pub fn delay_ms(ms: u64) -> Delay {
    delay(Duration::from_millis(ms))
}

pub struct Delay {
    nanos: i64,
    fired: Option<Rc<Cell<bool>>>,
}

impl Future for Delay {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        match &self.fired {
            Some(f) => {
                if f.get() {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            }
            None => {
                if self.nanos <= 0 {
                    return Poll::Ready(());
                }
                let (d, job) = current();
                let fired = Rc::new(Cell::new(false));
                d.schedule_wake(d.now().saturating_add(self.nanos), fired.clone(), cx.waker().clone(), &job);
                self.fired = Some(fired);
                Poll::Pending
            }
        }
    }
}

/// `await_frame().await`: waits for the next frame and gives its time in nanoseconds.
pub fn await_frame() -> AwaitFrame {
    AwaitFrame { time: None }
}

pub struct AwaitFrame {
    time: Option<Rc<Cell<Option<i64>>>>,
}

impl Future for AwaitFrame {
    type Output = i64;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<i64> {
        match &self.time {
            Some(t) => match t.get() {
                Some(time) => Poll::Ready(time),
                None => Poll::Pending,
            },
            None => {
                let (d, job) = current();
                let time = Rc::new(Cell::new(None));
                d.at_next_frame(time.clone(), cx.waker().clone(), &job);
                self.time = Some(time);
                Poll::Pending
            }
        }
    }
}

/// `await_frame_end().await`: waits until the frame under way (or the next one) is done composing and laying out,
/// and gives its time — where Compose's coroutines continue after a frame, their frame continuations being
/// dispatched after the frame's composition and layout passes (what reads the layout a frame's animation step
/// changed: ContentInViewNode's `afterFrame`). A coroutine resumed by a frame (`await_frame`) continues in it,
/// before its composition, so what it sets shows in that frame.
pub fn await_frame_end() -> AwaitFrameEnd {
    AwaitFrameEnd { time: None }
}

pub struct AwaitFrameEnd {
    time: Option<Rc<Cell<Option<i64>>>>,
}

impl Future for AwaitFrameEnd {
    type Output = i64;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<i64> {
        match &self.time {
            Some(t) => match t.get() {
                Some(time) => Poll::Ready(time),
                None => Poll::Pending,
            },
            None => {
                let (d, job) = current();
                let time = Rc::new(Cell::new(None));
                d.at_frame_end(time.clone(), cx.waker().clone(), &job);
                self.time = Some(time);
                Poll::Pending
            }
        }
    }
}

/// Compose's `withFrameNanos { time -> … }`: waits for the next frame and gives what `on_frame` makes of its time.
pub async fn with_frame_nanos<R>(on_frame: impl FnOnce(i64) -> R) -> R {
    on_frame(await_frame().await)
}

/// Compose's `withFrameMillis`.
pub async fn with_frame_millis<R>(on_frame: impl FnOnce(i64) -> R) -> R {
    on_frame(await_frame().await / 1_000_000)
}
