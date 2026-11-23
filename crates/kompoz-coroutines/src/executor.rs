//! The dispatcher that runs coroutines on the UI thread, and what it runs: launched coroutines (jobs) in scopes.
//!
//! A coroutine is a future. The dispatcher polls it on the UI thread: when it is launched as its
//! [`CoroutineStart`] says (by default dispatched: queued, polled when the dispatcher next runs what is ready, as
//! Kotlin's `launch`), then whenever it is woken — by a timer or a frame the host drives ([`Dispatcher::send_frame`],
//! [`Dispatcher::run_frame_end`]), or from any thread (a background result, a flow's value, a channel's element).
//! Cancelling one drops its future where it waits, running its destructors.
//!
//! Failures are panics: one ending a coroutine is caught there and goes where Kotlin's exception would — to a
//! structured scope (`coroutine_scope`), which cancels the rest and hands it to who awaits the scope; to who
//! awaits an `async`'s `Deferred`; or, with nobody to take it, reported (not fatal).

use crate::context::{CoroutineContext, push_context};
use kompoz_runtime::named;
use parking_lot::Mutex;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

/// What a failed coroutine panicked with.
pub struct Failure(pub(crate) Box<dyn Any + Send>);

impl Failure {
    /// The panic's message, if it had one.
    pub fn message(&self) -> String {
        if let Some(s) = self.0.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = self.0.downcast_ref::<String>() {
            s.clone()
        } else {
            "a panic without a message".into()
        }
    }
    /// Panics again with what it panicked with (where it is awaited).
    pub fn resume(self) -> ! {
        std::panic::resume_unwind(self.0)
    }
}

impl std::fmt::Debug for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Failure({:?})", self.message())
    }
}

/// What a coroutine failed with that nobody takes (a cancellation isn't a failure).
fn report(failure: &Failure) {
    eprintln!("kompoz: coroutine failed: {}", failure.message());
}

// ---------------------------------------------------------------------------------------------------------------
// Wakers

/// Tasks woken, from any thread, waiting to be polled on the UI thread.
pub(crate) struct Queue {
    ready: Mutex<Vec<(u32, u32)>>,
    /// Told when a task is woken (the host asks for a frame).
    notify: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

struct TaskWaker {
    queue: Arc<Queue>,
    index: u32,
    generation: u32,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.queue.ready.lock().push((self.index, self.generation));
        let notify = self.queue.notify.lock().clone();
        if let Some(n) = notify {
            n();
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Dispatcher

struct Slot {
    generation: u32,
    task: Option<Pin<Box<dyn Future<Output = ()>>>>,
    job: Option<Rc<JobState>>,
}

enum TimerAction {
    Wake(Rc<Cell<bool>>, Waker),
    Call(Box<dyn FnOnce()>),
}

struct Timer {
    when: i64,
    seq: u64,
    action: TimerAction,
    job: Weak<JobState>,
}

struct FrameWaiter {
    time: Rc<Cell<Option<i64>>>,
    waker: Waker,
    job: Weak<JobState>,
}

pub(crate) struct Inner {
    slots: RefCell<Vec<Slot>>,
    free: RefCell<Vec<u32>>,
    queue: Arc<Queue>,
    timers: RefCell<Vec<Timer>>,
    frames: RefCell<Vec<FrameWaiter>>,
    /// Who waits for the end of the next frame (after its composition and layout).
    frame_ends: RefCell<Vec<FrameWaiter>>,
    now: Cell<i64>,
    seq: Cell<u64>,
}

/// Runs coroutines on the UI thread: polls what is woken, fires timers and hands frames to who waits for them,
/// as the host drives it. Clones are the same dispatcher.
#[derive(Clone)]
pub struct Dispatcher(pub(crate) Rc<Inner>);

impl PartialEq for Dispatcher {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    /// The coroutine being polled: its dispatcher and job (innermost last).
    static CURRENT: RefCell<Vec<(Rc<Inner>, Rc<JobState>)>> = const { RefCell::new(Vec::new()) };
    /// The UI thread's main dispatcher: the one a host made last (see [`Dispatcher::make_main`]).
    static MAIN: RefCell<Weak<Inner>> = const { RefCell::new(Weak::new()) };
}

struct PopCurrent;
impl Drop for PopCurrent {
    fn drop(&mut self) {
        CURRENT.with(|c| c.borrow_mut().pop());
    }
}

/// The dispatcher and job of the coroutine being polled.
pub(crate) fn current() -> (Rc<Inner>, Rc<JobState>) {
    CURRENT.with(|c| c.borrow().last().cloned()).expect("kompoz: a suspending call outside of a coroutine")
}

impl Dispatcher {
    pub fn new() -> Dispatcher {
        Dispatcher(Rc::new(Inner {
            slots: RefCell::new(Vec::new()),
            free: RefCell::new(Vec::new()),
            queue: Arc::new(Queue { ready: Mutex::new(Vec::new()), notify: Mutex::new(None) }),
            timers: RefCell::new(Vec::new()),
            frames: RefCell::new(Vec::new()),
            frame_ends: RefCell::new(Vec::new()),
            now: Cell::new(0),
            seq: Cell::new(0),
        }))
    }

    /// The dispatcher of the coroutine running now.
    pub fn current() -> Dispatcher {
        Dispatcher(current().0)
    }

    /// Kotlin's `Dispatchers.Main` (and `Dispatchers.Main.immediate`: a KompozR dispatcher runs on the UI thread,
    /// where what calls this runs): the dispatcher of the coroutine running now, else this thread's main one — the
    /// one its host made last ([`make_main`](Self::make_main)), while it lives. What a lifecycle's or a ViewModel's
    /// coroutine scope launches on.
    ///
    /// # Panics
    /// Outside a coroutine on a thread without a host.
    pub fn main() -> Dispatcher {
        if let Some((inner, _)) = CURRENT.with(|c| c.borrow().last().cloned()) {
            return Dispatcher(inner);
        }
        MAIN.with(|m| m.borrow().upgrade())
            .map(Dispatcher)
            .expect("kompoz: no main dispatcher here (made by a host on the UI thread)")
    }

    /// Makes it this thread's main dispatcher ([`main`](Self::main)) while it lives: a host's, as it is made.
    pub fn make_main(&self) {
        MAIN.with(|m| *m.borrow_mut() = Rc::downgrade(&self.0));
    }

    /// Calls `notify` (on the waking thread) whenever a coroutine is woken: the host asks for a frame.
    pub fn set_notify(&self, notify: impl Fn() + Send + Sync + 'static) {
        *self.0.queue.notify.lock() = Some(Arc::new(notify));
    }

    /// The time of its clock, in nanoseconds: the last frame's (or a newer event's).
    pub fn now(&self) -> i64 {
        self.0.now.get()
    }

    /// Moves its clock to `time` if that is newer (an input event stamped after the last frame: delays and
    /// timeouts it starts count from it).
    pub fn advance_to(&self, time: i64) {
        if time > self.0.now.get() {
            self.0.now.set(time);
        }
    }

    /// A whole frame at `now` with nothing between its two halves: [`send_frame`](Self::send_frame), then
    /// [`run_frame_end`](Self::run_frame_end).
    pub fn run(&self, now: i64) {
        self.send_frame(now);
        self.run_frame_end();
    }

    /// The first half of a frame at `now`, before its composition and layout: the timers due fire and what is ready
    /// runs; then each coroutine awaiting the frame runs its frame callback (`with_frame_nanos`' `on_frame`) — and
    /// only that: what it does after is dispatched, queued to run at [`run_frame_end`](Self::run_frame_end), as are
    /// the coroutines its callback wakes and those the composition launches. This is Compose's frame: the
    /// Recomposer's `BroadcastFrameClock.sendFrame` calls the awaiters' `onFrame` in the frame, their continuations
    /// are resumed through their dispatcher (AndroidUiDispatcher, or the test's FrameDeferringContinuationInterceptor)
    /// after the frame's composition and layout.
    pub fn send_frame(&self, now: i64) {
        self.run_timers(now);
        let now = self.0.now.get();
        let waiters = std::mem::take(&mut *self.0.frames.borrow_mut());
        for w in waiters {
            if job_alive(&w.job) {
                w.time.set(Some(now));
                w.waker.wake();
            }
        }
        // (the awaiters just woken, each polled once: it runs its callback, wakes itself again and waits)
        let woken = std::mem::take(&mut *self.0.queue.ready.lock());
        for (index, generation) in woken {
            self.0.poll(index, generation);
        }
    }

    /// Time moved to `now` without a frame (a test clock advanced between frames): the timers due fire and everything
    /// woken runs; the coroutines waiting for a frame keep waiting.
    pub fn run_timers(&self, now: i64) {
        // (what was dispatched before runs at the time it was dispatched, as a test scheduler runs the tasks due
        // now before it moves the time)
        self.run_ready();
        self.advance_to(now);
        let now = self.0.now.get();
        let mut due: Vec<Timer> = Vec::new();
        self.0.timers.borrow_mut().retain_mut(|t| {
            if !job_alive(&t.job) {
                return false;
            }
            if t.when > now {
                return true;
            }
            due.push(Timer {
                when: t.when,
                seq: t.seq,
                action: std::mem::replace(&mut t.action, TimerAction::Call(Box::new(|| {}))),
                job: t.job.clone(),
            });
            false
        });
        due.sort_by_key(|t| (t.when, t.seq));
        for t in due {
            if !job_alive(&t.job) {
                continue;
            }
            match t.action {
                TimerAction::Wake(fired, waker) => {
                    fired.set(true);
                    waker.wake();
                }
                TimerAction::Call(f) => f(),
            }
            self.run_ready();
        }
        self.run_ready();
    }

    /// The end of a frame (the host calls it once the frame's composition and layout are done): the coroutines
    /// awaiting it ([`await_frame_end`](crate::await_frame_end)) get it, and everything woken runs — the frame's
    /// continuations first, then the coroutines launched meanwhile (the composition's effects), in that order.
    pub fn run_frame_end(&self) {
        let now = self.0.now.get();
        let waiters = std::mem::take(&mut *self.0.frame_ends.borrow_mut());
        for w in waiters {
            if job_alive(&w.job) {
                w.time.set(Some(now));
                w.waker.wake();
            }
        }
        self.run_ready();
    }

    /// Polls what was woken (from this thread or another) until nothing is left.
    pub fn run_ready(&self) {
        loop {
            let ready = std::mem::take(&mut *self.0.queue.ready.lock());
            if ready.is_empty() {
                return;
            }
            for (index, generation) in ready {
                self.0.poll(index, generation);
            }
        }
    }

    /// Runs `f`, then polls the coroutines it woke before returning — and those they wake in turn, until none is
    /// left — as Compose resumes the continuation awaiting a pointer event in place, while the event is dispatched,
    /// so the next handler sees what it consumed. Coroutines woken before `f` (from this thread or another), and
    /// those launched meanwhile (dispatched, not started yet), wait for the next `run` or `run_ready`.
    pub fn run_woken_by<R>(&self, f: impl FnOnce() -> R) -> R {
        let earlier = std::mem::take(&mut *self.0.queue.ready.lock());
        /// Puts the earlier ones back in front of what is left, however this ends (a panic in `f` included).
        struct Restore<'a>(&'a Inner, Vec<(u32, u32)>);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                if !self.1.is_empty() {
                    let mut ready = self.0.queue.ready.lock();
                    let later = std::mem::take(&mut *ready);
                    *ready = std::mem::take(&mut self.1);
                    ready.extend(later);
                }
            }
        }
        let restore = Restore(&self.0, earlier);
        let result = f();
        let mut busy = Vec::new();
        loop {
            let woken = std::mem::take(&mut *self.0.queue.ready.lock());
            if woken.is_empty() {
                break;
            }
            for (index, generation) in woken {
                if self.0.is_polling(index, generation) || !self.0.has_started(index, generation) {
                    // Being polled further up (this runs from inside a coroutine): its wake stays for later. Or
                    // launched meanwhile and not started yet: dispatched, it starts when the dispatcher next runs
                    // what is ready (only what `f` resumes goes on in place).
                    busy.push((index, generation));
                } else {
                    self.0.poll(index, generation);
                }
            }
        }
        self.0.queue.ready.lock().extend(busy);
        drop(restore);
        result
    }

    /// Whether a coroutine was woken and waits to be polled.
    pub fn has_ready(&self) -> bool {
        !self.0.queue.ready.lock().is_empty()
    }

    pub fn has_frame_waiters(&self) -> bool {
        self.0.frames.borrow().iter().chain(self.0.frame_ends.borrow().iter()).any(|w| job_alive(&w.job))
    }

    pub fn has_timers(&self) -> bool {
        self.0.timers.borrow().iter().any(|t| job_alive(&t.job))
    }

    /// When the next timer is due, if any.
    pub fn next_deadline(&self) -> Option<i64> {
        self.0.timers.borrow().iter().filter(|t| job_alive(&t.job)).map(|t| t.when).min()
    }

    /// How many coroutines are running (launched, not done).
    pub fn active_tasks(&self) -> usize {
        self.0.slots.borrow().iter().filter(|s| s.job.is_some()).count()
    }
}

fn job_alive(job: &Weak<JobState>) -> bool {
    job.upgrade().is_some_and(|j| !j.done.get())
}

impl Inner {
    fn insert(self: &Rc<Self>, task: Pin<Box<dyn Future<Output = ()>>>, job: Rc<JobState>) -> (u32, u32) {
        let mut slots = self.slots.borrow_mut();
        let index = match self.free.borrow_mut().pop() {
            Some(i) => i,
            None => {
                slots.push(Slot { generation: 0, task: None, job: None });
                (slots.len() - 1) as u32
            }
        };
        let slot = &mut slots[index as usize];
        slot.generation = slot.generation.wrapping_add(1);
        slot.task = Some(task);
        slot.job = Some(job);
        (index, slot.generation)
    }

    /// Takes the task out (its slot freed); dropped by the caller, nothing borrowed.
    fn remove(&self, index: u32, generation: u32) -> Option<Pin<Box<dyn Future<Output = ()>>>> {
        let mut slots = self.slots.borrow_mut();
        let slot = slots.get_mut(index as usize)?;
        if slot.generation != generation || slot.job.is_none() {
            return None;
        }
        slot.job = None;
        self.free.borrow_mut().push(index);
        slot.task.take()
    }

    /// Whether that task is being polled now (taken out of its slot further up the stack).
    /// Whether that task has been polled once (a dispatched launch waits for its first turn).
    fn has_started(&self, index: u32, generation: u32) -> bool {
        let slots = self.slots.borrow();
        slots
            .get(index as usize)
            .is_some_and(|s| s.generation == generation && s.job.as_ref().is_some_and(|j| j.started.get()))
    }

    fn is_polling(&self, index: u32, generation: u32) -> bool {
        let slots = self.slots.borrow();
        slots.get(index as usize).is_some_and(|s| {
            s.generation == generation && s.task.is_none() && s.job.as_ref().is_some_and(|j| j.running.get())
        })
    }

    fn waker(&self, index: u32, generation: u32) -> Waker {
        Waker::from(Arc::new(TaskWaker { queue: self.queue.clone(), index, generation }))
    }

    /// Polls a task once.
    fn poll(self: &Rc<Self>, index: u32, generation: u32) {
        let (mut task, job) = {
            let mut slots = self.slots.borrow_mut();
            let Some(slot) = slots.get_mut(index as usize) else { return };
            if slot.generation != generation {
                return;
            }
            let (Some(task), Some(job)) = (slot.task.take(), slot.job.clone()) else { return }; // (being polled)
            (task, job)
        };
        // (one started ATOMIC or UNDISPATCHED runs up to its first suspension, cancelled or not)
        let unstoppable = job.unstoppable.replace(false);
        if job.done.get() || (job.cancelled.get() && !unstoppable) {
            return;
        }
        let waker = self.waker(index, generation);
        // (a gate in its context holding it back: it waits there, its task kept, until the gate wakes it)
        if let Some(gate) = job.context.get::<crate::context::DispatchGate>()
            && !gate.may_run(&waker)
        {
            let mut slots = self.slots.borrow_mut();
            let slot = &mut slots[index as usize];
            if slot.generation == generation && slot.job.is_some() {
                slot.task = Some(task);
                return;
            }
            drop(slots);
            drop(task);
            return;
        }
        let mut cx = Context::from_waker(&waker);
        CURRENT.with(|c| c.borrow_mut().push((self.clone(), job.clone())));
        job.running.set(true);
        job.started.set(true);
        let result = {
            let _pop = PopCurrent;
            let _context = push_context(job.context.clone());
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task.as_mut().poll(&mut cx)))
        };
        job.running.set(false);
        match result {
            Ok(Poll::Pending) if !job.cancelled.get() => {
                let mut slots = self.slots.borrow_mut();
                let slot = &mut slots[index as usize];
                if slot.generation == generation && slot.job.is_some() {
                    slot.task = Some(task);
                    return;
                }
                drop(slots);
                drop(task);
            }
            Ok(Poll::Pending) => {
                // Cancelled from inside: its future goes now that it waits.
                drop(task);
                job.complete();
            }
            Ok(Poll::Ready(())) => {
                drop(task);
                job.complete();
            }
            Err(payload) => {
                drop(task);
                job.fail(Failure(payload));
            }
        }
    }

    fn schedule(&self, when: i64, action: TimerAction, job: &Rc<JobState>) {
        let seq = self.seq.get();
        self.seq.set(seq + 1);
        self.timers.borrow_mut().push(Timer { when, seq, action, job: Rc::downgrade(job) });
    }

    pub(crate) fn schedule_wake(&self, when: i64, fired: Rc<Cell<bool>>, waker: Waker, job: &Rc<JobState>) {
        self.schedule(when, TimerAction::Wake(fired, waker), job);
    }

    pub(crate) fn schedule_call(&self, when: i64, f: Box<dyn FnOnce()>, job: &Rc<JobState>) {
        self.schedule(when, TimerAction::Call(f), job);
    }

    pub(crate) fn at_next_frame(&self, time: Rc<Cell<Option<i64>>>, waker: Waker, job: &Rc<JobState>) {
        self.frames.borrow_mut().push(FrameWaiter { time, waker, job: Rc::downgrade(job) });
    }

    pub(crate) fn at_frame_end(&self, time: Rc<Cell<Option<i64>>>, waker: Waker, job: &Rc<JobState>) {
        self.frame_ends.borrow_mut().push(FrameWaiter { time, waker, job: Rc::downgrade(job) });
    }

    pub(crate) fn now(&self) -> i64 {
        self.now.get()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Jobs and scopes

/// A launched coroutine's state.
pub(crate) struct JobState {
    dispatcher: Weak<Inner>,
    task: Cell<Option<(u32, u32)>>,
    scope: RefCell<Weak<ScopeState>>,
    running: Cell<bool>,
    cancelled: Cell<bool>,
    pub(crate) done: Cell<bool>,
    /// Launched `LAZY` and not started yet (Kotlin's "new" state): it starts on `start`, `join` or `await`.
    lazy: Cell<bool>,
    /// Polled at least once.
    started: Cell<bool>,
    /// Launched `ATOMIC` or `UNDISPATCHED` and not polled yet: it runs up to its first suspension even if it is
    /// cancelled first.
    unstoppable: Cell<bool>,
    /// An async's: what it fails with goes to who awaits it.
    deferred: bool,
    /// A scope's own block: what it fails with fails the scope, a supervisor's too.
    body: bool,
    failure: RefCell<Option<Failure>>,
    joiners: RefCell<Vec<Waker>>,
    /// Told once it ends (Kotlin's `Job.invokeOnCompletion`), unless disposed first.
    on_completion: RefCell<Vec<(Rc<Cell<bool>>, Box<dyn FnOnce()>)>>,
    /// Its scope's context (what it reads as `current_coroutine_context`).
    context: CoroutineContext,
}

impl JobState {
    fn new(dispatcher: &Rc<Inner>, scope: &Rc<ScopeState>, deferred: bool, body: bool) -> Rc<JobState> {
        Rc::new(JobState {
            context: scope.context.clone(),
            dispatcher: Rc::downgrade(dispatcher),
            task: Cell::new(None),
            scope: RefCell::new(Rc::downgrade(scope)),
            running: Cell::new(false),
            cancelled: Cell::new(false),
            done: Cell::new(false),
            lazy: Cell::new(false),
            started: Cell::new(false),
            unstoppable: Cell::new(false),
            deferred,
            body,
            failure: RefCell::new(None),
            joiners: RefCell::new(Vec::new()),
            on_completion: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn cancel(self: &Rc<Self>) {
        if self.done.get() || self.cancelled.get() {
            return;
        }
        self.cancelled.set(true);
        if self.running.get() {
            return; // cancelled from inside: dropped once it waits
        }
        if self.unstoppable.get() {
            return; // (ATOMIC, UNDISPATCHED: it runs up to its first suspension, then goes)
        }
        self.complete();
    }

    /// Dispatched: queued on its dispatcher, polled when it next runs what is ready.
    fn dispatch(&self) {
        if let (Some((i, g)), Some(d)) = (self.task.get(), self.dispatcher.upgrade()) {
            d.waker(i, g).wake();
        }
    }

    /// Starts it if it was launched `LAZY` and isn't started yet (Kotlin's `Job.start()`): dispatched, as Kotlin's
    /// lazy coroutine is once started. Whether this call started it.
    fn start(&self) -> bool {
        if !self.lazy.replace(false) || self.done.get() {
            return false;
        }
        self.dispatch();
        true
    }

    fn fail(self: &Rc<Self>, failure: Failure) {
        let scope = self.scope.borrow().upgrade();
        match scope {
            Some(s) if s.structured => {
                s.child_failed(self, failure);
            }
            _ if self.deferred => *self.failure.borrow_mut() = Some(failure),
            _ => report(&failure),
        }
        self.complete();
    }

    /// Ended: its future gone, who waits for it woken, out of its scope.
    pub(crate) fn complete(self: &Rc<Self>) {
        if self.done.get() {
            return;
        }
        self.done.set(true);
        if let (Some((i, g)), Some(d)) = (self.task.take(), self.dispatcher.upgrade()) {
            let task = d.remove(i, g);
            drop(task); // (awaiters in it go: a coroutine scope's children are cancelled)
        }
        for w in std::mem::take(&mut *self.joiners.borrow_mut()) {
            w.wake();
        }
        let handlers = std::mem::take(&mut *self.on_completion.borrow_mut());
        for (disposed, handler) in handlers {
            if !disposed.get() {
                handler();
            }
        }
        let scope = self.scope.borrow().upgrade();
        if let Some(s) = scope {
            s.jobs.borrow_mut().retain(|j| !Rc::ptr_eq(j, self));
            s.left();
        }
    }
}

pub(crate) struct ScopeState {
    dispatcher: Weak<Inner>,
    jobs: RefCell<Vec<Rc<JobState>>>,
    cancelled: Cell<bool>,
    /// A coroutine scope's: a child failing fails it, cancelling the others (a supervisor's: only its block does).
    structured: bool,
    supervisor: bool,
    failure: RefCell<Option<Failure>>,
    /// Woken once its last child is done.
    when_empty: RefCell<Option<Waker>>,
    cancelling: Cell<bool>,
    /// What its coroutines run with.
    context: CoroutineContext,
    /// Told once it is cancelled and its last coroutine is done (Kotlin's job's `invokeOnCompletion`), unless
    /// disposed first.
    on_completion: RefCell<Vec<(Rc<Cell<bool>>, Box<dyn FnOnce()>)>>,
}

impl ScopeState {
    pub(crate) fn new(
        dispatcher: &Rc<Inner>,
        structured: bool,
        supervisor: bool,
        context: CoroutineContext,
    ) -> Rc<ScopeState> {
        Rc::new(ScopeState {
            context,
            dispatcher: Rc::downgrade(dispatcher),
            jobs: RefCell::new(Vec::new()),
            cancelled: Cell::new(false),
            structured,
            supervisor,
            failure: RefCell::new(None),
            when_empty: RefCell::new(None),
            cancelling: Cell::new(false),
            on_completion: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn cancel(&self, except: Option<&Rc<JobState>>) {
        self.cancelled.set(true);
        self.cancelling.set(true); // (who awaits it is woken once all of them are gone, not after the first)
        let all = self.jobs.borrow().clone();
        for job in all {
            if except.is_none_or(|e| !Rc::ptr_eq(e, &job)) {
                job.cancel();
            }
        }
        self.cancelling.set(false);
        self.left();
    }

    fn child_failed(&self, child: &Rc<JobState>, failure: Failure) {
        if self.supervisor && !child.body {
            report(&failure); // a supervisor's children fail on their own
            return;
        }
        if self.failure.borrow().is_none() {
            *self.failure.borrow_mut() = Some(failure);
        }
        self.cancel(Some(child));
    }

    /// A child is done.
    fn left(&self) {
        if !self.cancelling.get() && self.jobs.borrow().is_empty() {
            let w = self.when_empty.borrow_mut().take();
            if let Some(w) = w {
                w.wake();
            }
            if self.cancelled.get() {
                let handlers = std::mem::take(&mut *self.on_completion.borrow_mut());
                for (disposed, handler) in handlers {
                    if !disposed.get() {
                        handler();
                    }
                }
            }
        }
    }

    /// Whether it is cancelled and its coroutines are done (Kotlin's job completed).
    fn is_completed(&self) -> bool {
        self.cancelled.get() && !self.cancelling.get() && self.jobs.borrow().is_empty()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.jobs.borrow().is_empty()
    }

    pub(crate) fn set_when_empty(&self, waker: Waker) {
        *self.when_empty.borrow_mut() = Some(waker);
    }

    pub(crate) fn take_failure(&self) -> Option<Failure> {
        self.failure.borrow_mut().take()
    }

    /// Launches `future` here as `start` says (Kotlin's `launch(start = …)`): `DEFAULT` queued on the dispatcher,
    /// `UNDISPATCHED` run at once up to its first suspension, `ATOMIC` queued and run up to its first suspension
    /// even if cancelled before, `LAZY` started on `start`/`join`/`await`. In a cancelled scope, a `DEFAULT` or `LAZY`
    /// one is cancelled at once and never runs; an `ATOMIC` or `UNDISPATCHED` one runs, cancelled, up to its first
    /// suspension (Kotlin's: they start "even if the coroutine was already cancelled").
    pub(crate) fn launch(
        self: &Rc<Self>,
        future: Pin<Box<dyn Future<Output = ()>>>,
        deferred: bool,
        body: bool,
        start: CoroutineStart,
    ) -> Rc<JobState> {
        let d = self.dispatcher.upgrade().expect("kompoz: a scope whose dispatcher is gone");
        let job = JobState::new(&d, self, deferred, body);
        let unstoppable = matches!(start, CoroutineStart::ATOMIC | CoroutineStart::UNDISPATCHED);
        if self.cancelled.get() {
            job.cancelled.set(true);
            if !unstoppable {
                job.done.set(true);
                return job;
            }
        }
        let (i, g) = d.insert(future, job.clone());
        job.task.set(Some((i, g)));
        job.unstoppable.set(unstoppable);
        self.jobs.borrow_mut().push(job.clone());
        match start {
            CoroutineStart::DEFAULT | CoroutineStart::ATOMIC => job.dispatch(),
            CoroutineStart::UNDISPATCHED => d.poll(i, g),
            CoroutineStart::LAZY => job.lazy.set(true),
        }
        job
    }
}

/// Where coroutines are launched: they are cancelled together when it is (Kotlin's CoroutineScope). Clones are
/// the same scope.
#[derive(Clone)]
pub struct CoroutineScope(pub(crate) Rc<ScopeState>);

impl PartialEq for CoroutineScope {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl CoroutineScope {
    /// A scope of its own on `dispatcher` (a ViewModel's, a screen's): cancel it when it is done with.
    pub fn new(dispatcher: &Dispatcher) -> CoroutineScope {
        CoroutineScope::new_with_context(dispatcher, CoroutineContext::new())
    }

    /// The same, its coroutines run with `context` (Kotlin's `CoroutineScope(context)`).
    pub fn new_with_context(dispatcher: &Dispatcher, context: CoroutineContext) -> CoroutineScope {
        CoroutineScope(ScopeState::new(&dispatcher.0, false, false, context))
    }

    /// A scope whose coroutines fail on their own (Kotlin's `CoroutineScope(SupervisorJob() + context)`: a
    /// lifecycle's, a ViewModel's): one failing is reported and cancels none of the others.
    pub fn new_supervisor(dispatcher: &Dispatcher, context: CoroutineContext) -> CoroutineScope {
        CoroutineScope(ScopeState::new(&dispatcher.0, false, true, context))
    }

    /// What its coroutines run with.
    pub fn coroutine_context(&self) -> CoroutineContext {
        self.0.context.clone()
    }

    /// Cancels everything launched here; nothing launched afterwards runs.
    pub fn cancel(&self) {
        self.0.cancel(None);
    }

    pub fn is_active(&self) -> bool {
        !self.0.cancelled.get()
    }

    /// Calls `handler` once the scope is cancelled and its coroutines are done (Kotlin's
    /// `coroutineContext[Job]?.invokeOnCompletion { … }` on a scope's job), at once if that is so already; dispose
    /// the handle to take it back.
    pub fn invoke_on_completion(&self, handler: impl FnOnce() + 'static) -> DisposableHandle {
        let disposed = Rc::new(Cell::new(false));
        if self.0.is_completed() {
            handler();
            disposed.set(true);
        } else {
            self.0.on_completion.borrow_mut().push((disposed.clone(), Box::new(handler)));
        }
        DisposableHandle(disposed)
    }

    /// How many of its coroutines are running.
    pub fn active_jobs(&self) -> usize {
        self.0.jobs.borrow().len()
    }

    pub fn dispatcher(&self) -> Dispatcher {
        Dispatcher(self.0.dispatcher.upgrade().expect("kompoz: a scope whose dispatcher is gone"))
    }
}

#[named]
impl CoroutineScope {
    /// Launches `coroutine` here (Kotlin's `launch`), as `start` says: by default dispatched — queued on the
    /// dispatcher, it starts when the dispatcher next runs what is ready (after the frame's composition and layout,
    /// for one the composition launches; once the event being handled is done, for one an event handler launches);
    /// see [`CoroutineStart`] for the others.
    ///
    /// Kotlin: `launch(start: CoroutineStart = CoroutineStart.DEFAULT, block: suspend CoroutineScope.() -> Unit): Job`
    pub fn launch(
        &self,
        #[default(CoroutineStart::DEFAULT)] start: CoroutineStart,
        coroutine: impl Future<Output = ()> + 'static,
    ) -> Job {
        Job(self.0.launch(Box::pin(coroutine), false, false, start))
    }

    /// Launches `coroutine` for the value it gives (Kotlin's `async`), as `start` says (as [`launch`](Self::launch)):
    /// await the [`Deferred`] for it.
    ///
    /// Kotlin: `async(start: CoroutineStart = CoroutineStart.DEFAULT, block: suspend CoroutineScope.() -> T): Deferred<T>`
    pub fn r#async<T: 'static>(
        &self,
        #[default(CoroutineStart::DEFAULT)] start: CoroutineStart,
        coroutine: impl Future<Output = T> + 'static,
    ) -> Deferred<T> {
        let value: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
        let slot = value.clone();
        let job = self.0.launch(
            Box::pin(async move {
                let v = coroutine.await;
                *slot.borrow_mut() = Some(v);
            }),
            true,
            false,
            start,
        );
        Deferred { job: Job(job), value }
    }
}

/// What takes a handler back (Kotlin's DisposableHandle): [`CoroutineScope::invoke_on_completion`]'s,
/// [`Job::invoke_on_completion`]'s.
#[derive(Clone)]
pub struct DisposableHandle(Rc<Cell<bool>>);

impl DisposableHandle {
    /// The handler won't be called (Kotlin's `dispose()`).
    pub fn dispose(&self) {
        self.0.set(true);
    }
}

/// How a coroutine is launched (Kotlin's CoroutineStart): [`CoroutineScope::launch`]'s and `async`'s `start`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum CoroutineStart {
    /// Dispatched: queued on its dispatcher, it starts when the dispatcher next runs what is ready; cancelled before
    /// that, it never runs.
    #[default]
    DEFAULT,
    /// Started only when asked: [`Job::start`], [`Job::join`] or awaiting its [`Deferred`] (then dispatched);
    /// cancelled before, it never runs.
    LAZY,
    /// Dispatched as `DEFAULT`, but not cancellable before it starts: it runs up to its first suspension even if
    /// cancelled meanwhile.
    ATOMIC,
    /// Run at once, in the caller, up to its first suspension (even if already cancelled); dispatched as usual
    /// once resumed from there.
    UNDISPATCHED,
}

/// A launched coroutine (Kotlin's Job): cancel it, ask how it is, or await [`join`](Job::join) for it to end.
#[derive(Clone)]
pub struct Job(pub(crate) Rc<JobState>);

impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Job {
    pub fn cancel(&self) {
        self.0.cancel();
    }
    /// Starts it if it was launched `LAZY` and not started yet (Kotlin's `start()`): whether this call started it.
    pub fn start(&self) -> bool {
        self.0.start()
    }
    /// Running: started (not a `LAZY` one waiting to), not completed nor cancelled.
    pub fn is_active(&self) -> bool {
        !self.0.done.get() && !self.0.cancelled.get() && !self.0.lazy.get()
    }
    /// Ended, however (cancelled too).
    pub fn is_completed(&self) -> bool {
        self.0.done.get()
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.get()
    }
    /// Calls `handler` once it ends (completed, failed or cancelled: Kotlin's `Job.invokeOnCompletion { … }`, its
    /// cause not given, as [`CoroutineScope::invoke_on_completion`]'s), at once if it has; dispose the handle to take
    /// it back.
    pub fn invoke_on_completion(&self, handler: impl FnOnce() + 'static) -> DisposableHandle {
        let disposed = Rc::new(Cell::new(false));
        if self.0.done.get() {
            handler();
            disposed.set(true);
        } else {
            self.0.on_completion.borrow_mut().push((disposed.clone(), Box::new(handler)));
        }
        DisposableHandle(disposed)
    }
    /// Waits until it ends (completed, failed or cancelled); a `LAZY` one not started yet is started first.
    pub fn join(&self) -> impl Future<Output = ()> + 'static {
        let job = self.0.clone();
        let mut started = false;
        std::future::poll_fn(move |cx| {
            if !started {
                started = true;
                job.start();
            }
            if job.done.get() {
                Poll::Ready(())
            } else {
                job.joiners.borrow_mut().push(cx.waker().clone());
                Poll::Pending
            }
        })
    }
}

/// An `async`'s coroutine and the value it will give (Kotlin's Deferred): `.await` it for the value — `Err` if it
/// was cancelled; a panic it ended with panics again where it is awaited.
pub struct Deferred<T> {
    job: Job,
    value: Rc<RefCell<Option<T>>>,
}

impl<T> Clone for Deferred<T> {
    fn clone(&self) -> Self {
        Deferred { job: self.job.clone(), value: self.value.clone() }
    }
}

/// An awaited coroutine was cancelled before it gave its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("the coroutine was cancelled")
    }
}
impl std::error::Error for Cancelled {}

impl<T> Deferred<T> {
    pub fn job(&self) -> &Job {
        &self.job
    }
    /// Starts it if it was launched `LAZY` and not started yet (Kotlin's `start()`): whether this call started it.
    pub fn start(&self) -> bool {
        self.job.start()
    }
}

impl<T: 'static> Deferred<T> {
    /// Its value, once it is there (Kotlin's `await()`; a `LAZY` one is started): a failure panics again here; a
    /// cancelled one cancels the awaiting coroutine (Kotlin's CancellationException). `.await` on the `Deferred`
    /// itself gives `Err(Cancelled)` instead.
    ///
    /// Kotlin: `await(): T`
    pub async fn r#await(self) -> T {
        match std::future::IntoFuture::into_future(self).await {
            Ok(value) => value,
            Err(Cancelled) => cancel_current().await,
        }
    }
}

impl<T: 'static> std::future::IntoFuture for Deferred<T> {
    type Output = Result<T, Cancelled>;
    type IntoFuture = Pin<Box<dyn Future<Output = Result<T, Cancelled>>>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.job.join().await;
            if let Some(v) = self.value.borrow_mut().take() {
                return Ok(v);
            }
            let failure = self.job.0.failure.borrow_mut().take();
            if let Some(f) = failure {
                f.resume();
            }
            Err(Cancelled)
        })
    }
}

/// The coroutine running now (Kotlin's `coroutineContext[Job]`).
pub fn current_job() -> Job {
    Job(current().1)
}

/// Cancels the coroutine running now and never gives a value: what Kotlin's `throw CancellationException()` does
/// where nothing catches it (the coroutine ends there, its destructors run; awaited by `coroutine_scope`, the
/// awaiting coroutine is cancelled too).
pub async fn cancel_current<T>() -> T {
    current_job().cancel();
    std::future::pending().await
}

/// Lets what else is ready run first (Kotlin's `yield()`): the coroutine goes on when its dispatcher next runs what
/// is ready, after what is queued before it. A coroutine cancelled meanwhile goes no further.
///
/// Kotlin: `yield()`
pub async fn r#yield() {
    let mut yielded = false;
    std::future::poll_fn(move |cx| {
        if yielded {
            return Poll::Ready(());
        }
        yielded = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    })
    .await;
}

/// Runs `future`, giving what it failed with instead of failing (Kotlin's `runCatching`, a `try`/`catch` around
/// suspending calls): `Err` holds the panic.
pub async fn run_catching<T>(future: impl Future<Output = T>) -> Result<T, Failure> {
    let mut future = Box::pin(future);
    std::future::poll_fn(move |cx| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| future.as_mut().poll(cx))) {
            Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(Failure(payload))),
        }
    })
    .await
}
