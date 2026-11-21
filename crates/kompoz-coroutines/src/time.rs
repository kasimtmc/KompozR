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
/// `Long.MAX_VALUE` milliseconds (or a time as long, `Duration::MAX`) waits forever, no timer scheduled, as
/// kotlinx's delay (WP5: a snackbar of an indefinite duration waits so).
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
                if self.nanos == i64::MAX {
                    // (kotlinx: `if (timeMillis < Long.MAX_VALUE) scheduleResumeAfterDelay(…)`: forever)
                    self.fired = Some(Rc::new(Cell::new(false)));
                    return Poll::Pending;
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

/// `await_frame().await`: waits for the next frame and gives its time in nanoseconds — `with_frame_nanos { it }`:
/// the coroutine goes on once the frame is done composing and laying out (see [`with_frame_nanos`]).
pub fn await_frame() -> AwaitFrame {
    AwaitFrame { time: None, framed: false }
}

pub struct AwaitFrame {
    time: Option<Rc<Cell<Option<i64>>>>,
    /// Seen the frame once (its callback's turn, in the frame): the next poll is the continuation's.
    framed: bool,
}

/// Where a coroutine awaiting a frame is.
enum FramePoll {
    Waiting,
    /// In the frame (`send_frame`): the frame callback's turn; the coroutine has woken itself to go on after.
    InFrame(i64),
    /// After the frame's composition and layout: the continuation's turn.
    Continue(i64),
}

impl AwaitFrame {
    fn poll_frame(&mut self, cx: &mut Context<'_>) -> FramePoll {
        match &self.time {
            Some(t) => match t.get() {
                Some(time) if self.framed => FramePoll::Continue(time),
                Some(time) => {
                    self.framed = true;
                    cx.waker().wake_by_ref();
                    FramePoll::InFrame(time)
                }
                None => FramePoll::Waiting,
            },
            None => {
                let (d, job) = current();
                let time = Rc::new(Cell::new(None));
                d.at_next_frame(time.clone(), cx.waker().clone(), &job);
                self.time = Some(time);
                FramePoll::Waiting
            }
        }
    }
}

impl Future for AwaitFrame {
    type Output = i64;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<i64> {
        match self.poll_frame(cx) {
            FramePoll::Waiting | FramePoll::InFrame(_) => Poll::Pending,
            FramePoll::Continue(time) => Poll::Ready(time),
        }
    }
}

/// `await_frame_end().await`: waits until the frame under way (or the next one) is done composing and laying out,
/// and gives its time — where Compose's coroutines continue after a frame, their frame continuations being
/// dispatched after the frame's composition and layout passes (what reads the layout a frame's animation step
/// changed: ContentInViewNode's `afterFrame`). A coroutine awaiting a frame ([`with_frame_nanos`], `await_frame`)
/// runs its frame callback before the frame's composition and continues after its layout too, ahead of these.
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
/// As Compose's frame clock does it: `on_frame` runs in the frame, before its composition (what it sets shows in
/// that frame); the coroutine goes on after the frame's composition and layout (its continuation is dispatched), so
/// what it sets from there shows in the next frame.
pub async fn with_frame_nanos<R>(on_frame: impl FnOnce(i64) -> R) -> R {
    let mut frame = await_frame();
    let mut on_frame = Some(on_frame);
    let mut result = None;
    std::future::poll_fn(move |cx| match frame.poll_frame(cx) {
        FramePoll::Waiting => Poll::Pending,
        FramePoll::InFrame(time) => {
            result = on_frame.take().map(|f| f(time));
            Poll::Pending
        }
        FramePoll::Continue(time) => match result.take() {
            Some(r) => Poll::Ready(r),
            None => Poll::Ready((on_frame.take().expect("with_frame_nanos: polled after it gave its value"))(time)),
        },
    })
    .await
}

/// Compose's `withFrameMillis`.
pub async fn with_frame_millis<R>(on_frame: impl FnOnce(i64) -> R) -> R {
    with_frame_nanos(move |nanos| on_frame(nanos / 1_000_000)).await
}
