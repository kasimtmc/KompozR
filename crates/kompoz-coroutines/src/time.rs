//! Waiting in a coroutine: for a time on the dispatcher's clock, or for the next frame.

use crate::executor::current;
use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

/// `delay(duration).await`: waits that long on the dispatcher's clock (the frames' time).
pub fn delay(duration: Duration) -> Delay {
    Delay { nanos: duration.as_nanos().min(i64::MAX as u128) as i64, fired: None }
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

/// Compose's `withFrameNanos { time -> … }`: waits for the next frame and gives what `on_frame` makes of its time.
pub async fn with_frame_nanos<R>(on_frame: impl FnOnce(i64) -> R) -> R {
    on_frame(await_frame().await)
}

/// Compose's `withFrameMillis`.
pub async fn with_frame_millis<R>(on_frame: impl FnOnce(i64) -> R) -> R {
    on_frame(await_frame().await / 1_000_000)
}
