//! The Windows platform's loop's waiting (C++'s `run`'s `MsgWaitForMultipleObjectsEx`, Linux's `event_loop`): the
//! thread's message queue, a wake event the hosts' wakers set from any thread, and a high-resolution waitable timer
//! for what is due (a frame paced to the display, a coroutine's delay) — the one wait for all the app's windows,
//! which share the thread. Ctrl+C in the console the program runs from is a quit, as a signal is on Linux.

use super::sys::{self, EventHandle, WaitableTimer, Woken};
use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

/// What woke the loop.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Ready {
    /// A host's waker (or Ctrl+C) set the wake event.
    pub woken: bool,
    /// The deadline set ([`EventLoop::set_deadline`]) came.
    pub deadline: bool,
    /// Messages came (or were there already).
    pub messages: bool,
}

/// The wake event a host's waker sets (Send + Sync: any thread).
#[derive(Clone, Debug)]
pub struct Waker(EventHandle);

impl Waker {
    /// As the host's callbacks give it.
    pub fn as_fn(&self) -> Arc<dyn Fn() + Send + Sync> {
        let event = self.0.clone();
        Arc::new(move || event.set())
    }
}

/// The loop's wait: messages, the wake event, the timer.
pub struct EventLoop {
    wake: EventHandle,
    timer: WaitableTimer,
    /// When the timer was last set for (the platform's clock, ns; none: not set).
    armed: Cell<Option<i64>>,
    /// A deadline of the cores' (the earliest coroutine delay), on the platform's clock.
    deadline: Cell<Option<i64>>,
}

impl EventLoop {
    /// The wake event and the timer, Ctrl+C turned into a wake (and [`take_quit_signal`](Self::take_quit_signal));
    /// why not where the system can't make them.
    pub fn new() -> Result<EventLoop, String> {
        let wake = EventHandle::new().ok_or_else(|| format!("no event ({})", sys::last_error()))?;
        let timer = WaitableTimer::new().ok_or_else(|| format!("no waitable timer ({})", sys::last_error()))?;
        sys::on_ctrl_c(&wake);
        Ok(EventLoop { wake, timer, armed: Cell::new(None), deadline: Cell::new(None) })
    }

    pub fn waker(&self) -> Waker {
        Waker(self.wake.clone())
    }

    /// The loop woken at `at` (the platform's clock, ns) unless something wakes it before; none: no deadline.
    pub fn set_deadline(&self, at: Option<i64>) {
        self.deadline.set(at);
    }

    /// Whether Ctrl+C (or Ctrl+Break) came since last asked.
    pub fn take_quit_signal(&self) -> bool {
        sys::take_ctrl_c()
    }

    /// Up to `timeout` (none: no limit but the deadline) for a message, a wake or the deadline; `now` the
    /// platform's clock.
    pub fn wait(&self, timeout: Option<Duration>, now: fn() -> i64) -> Ready {
        let at = now();
        let until = timeout.map(|t| at.saturating_add(t.as_nanos().min(i64::MAX as u128) as i64));
        let wake_at = match (until, self.deadline.get()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        match wake_at {
            Some(t) if t <= at => return self.due(at),
            Some(t) => {
                self.timer.set_in(t - at);
                self.armed.set(Some(t));
            }
            None => {
                if self.armed.take().is_some() {
                    self.timer.cancel();
                }
            }
        }
        match sys::wait_for_messages(&self.wake, &self.timer) {
            Woken::First => Ready { woken: true, ..self.due(now()) },
            Woken::Second => {
                self.armed.set(None);
                self.due(now())
            }
            Woken::Messages => Ready { messages: true, ..self.due(now()) },
            Woken::Failed => self.due(now()),
        }
    }

    /// Whether the deadline has come at `at` (taken: it is the cores' to set again).
    fn due(&self, at: i64) -> Ready {
        let deadline = self.deadline.get().is_some_and(|d| d <= at);
        if deadline {
            self.deadline.set(None);
        }
        Ready { deadline, ..Ready::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn now() -> i64 {
        crate::windows::now()
    }

    #[test]
    fn a_wake_from_another_thread_ends_the_wait() {
        let lp = EventLoop::new().unwrap();
        let waker = lp.waker();
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            waker.as_fn()();
        });
        let ready = lp.wait(Some(Duration::from_secs(10)), now);
        assert!(ready.woken);
        t.join().unwrap();
    }

    #[test]
    fn timeouts_and_deadlines_end_it_on_time() {
        let lp = EventLoop::new().unwrap();
        let start = Instant::now();
        let ready = lp.wait(Some(Duration::from_millis(20)), now);
        let took = start.elapsed();
        assert_eq!(ready, Ready::default());
        assert!(took >= Duration::from_millis(19) && took < Duration::from_millis(500), "{took:?}");
        lp.set_deadline(Some(now() + 15_000_000));
        let ready = lp.wait(None, now);
        assert!(ready.deadline);
        // A deadline past: at once, taken.
        lp.set_deadline(Some(now() - 1));
        assert!(lp.wait(None, now).deadline);
        lp.set_deadline(None);
        let ready = lp.wait(Some(Duration::from_millis(1)), now);
        assert!(!ready.deadline);
    }

    #[test]
    fn a_posted_message_ends_it() {
        let lp = EventLoop::new().unwrap();
        // (A thread message: WM_USER posted to the thread's own queue through a hidden window.)
        assert!(sys::register_class("KompozTestLoop"));
        struct Nothing;
        impl sys::WindowHandler for Nothing {
            fn message(&self, _: sys::Hwnd, _: &sys::WindowMessage) -> Option<isize> {
                None
            }
        }
        let hwnd = sys::create_hidden_window("KompozTestLoop", std::rc::Rc::new(Nothing)).unwrap();
        hwnd.post(0x0400, 0, 0);
        let ready = lp.wait(Some(Duration::from_secs(10)), now);
        assert!(ready.messages);
        while let Some(m) = sys::peek_message() {
            sys::dispatch(&m);
        }
        hwnd.destroy();
    }
}
