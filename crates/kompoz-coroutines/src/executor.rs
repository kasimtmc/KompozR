//! The dispatcher that runs coroutines on the UI thread, and what it runs: launched coroutines (jobs) in scopes.
//!
//! A coroutine is a future. The dispatcher polls it on the UI thread: at once when it is launched (it runs until
//! it first waits), then whenever it is woken — by a timer or a frame the host drives ([`Dispatcher::run`]), or
//! from any thread (a background result, a flow's value, a channel's element). Cancelling one drops its future
//! where it waits, running its destructors.
//!
//! Failures are panics: one ending a coroutine is caught there and goes where Kotlin's exception would — to a
//! structured scope (`coroutine_scope`), which cancels the rest and hands it to who awaits the scope; to who
//! awaits an `async`'s `Deferred`; or, with nobody to take it, reported (not fatal).

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
            now: Cell::new(0),
            seq: Cell::new(0),
        }))
    }

    /// The dispatcher of the coroutine running now.
    pub fn current() -> Dispatcher {
        Dispatcher(current().0)
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

    /// A frame at `now`: the timers due fire, the coroutines waiting for a frame get it, and everything woken runs.
    pub fn run(&self, now: i64) {
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
        let waiters = std::mem::take(&mut *self.0.frames.borrow_mut());
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

    /// Whether a coroutine was woken and waits to be polled.
    pub fn has_ready(&self) -> bool {
        !self.0.queue.ready.lock().is_empty()
    }

    pub fn has_frame_waiters(&self) -> bool {
        self.0.frames.borrow().iter().any(|w| job_alive(&w.job))
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
        if job.done.get() || job.cancelled.get() {
            return;
        }
        let waker = self.waker(index, generation);
        let mut cx = Context::from_waker(&waker);
        CURRENT.with(|c| c.borrow_mut().push((self.clone(), job.clone())));
        job.running.set(true);
        let result = {
            let _pop = PopCurrent;
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
    /// An async's: what it fails with goes to who awaits it.
    deferred: bool,
    /// A scope's own block: what it fails with fails the scope, a supervisor's too.
    body: bool,
    failure: RefCell<Option<Failure>>,
    joiners: RefCell<Vec<Waker>>,
}

impl JobState {
    fn new(dispatcher: &Rc<Inner>, scope: &Rc<ScopeState>, deferred: bool, body: bool) -> Rc<JobState> {
        Rc::new(JobState {
            dispatcher: Rc::downgrade(dispatcher),
            task: Cell::new(None),
            scope: RefCell::new(Rc::downgrade(scope)),
            running: Cell::new(false),
            cancelled: Cell::new(false),
            done: Cell::new(false),
            deferred,
            body,
            failure: RefCell::new(None),
            joiners: RefCell::new(Vec::new()),
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
        self.complete();
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
}

impl ScopeState {
    pub(crate) fn new(dispatcher: &Rc<Inner>, structured: bool, supervisor: bool) -> Rc<ScopeState> {
        Rc::new(ScopeState {
            dispatcher: Rc::downgrade(dispatcher),
            jobs: RefCell::new(Vec::new()),
            cancelled: Cell::new(false),
            structured,
            supervisor,
            failure: RefCell::new(None),
            when_empty: RefCell::new(None),
            cancelling: Cell::new(false),
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
        }
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

    /// Starts `future` here (nothing if it's cancelled): it runs until it first waits.
    pub(crate) fn launch(
        self: &Rc<Self>,
        future: Pin<Box<dyn Future<Output = ()>>>,
        deferred: bool,
        body: bool,
    ) -> Rc<JobState> {
        let d = self.dispatcher.upgrade().expect("kompoz: a scope whose dispatcher is gone");
        let job = JobState::new(&d, self, deferred, body);
        if self.cancelled.get() {
            job.cancelled.set(true);
            job.done.set(true);
            return job;
        }
        let (i, g) = d.insert(future, job.clone());
        job.task.set(Some((i, g)));
        self.jobs.borrow_mut().push(job.clone());
        d.poll(i, g);
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
        CoroutineScope(ScopeState::new(&dispatcher.0, false, false))
    }

    /// Starts `coroutine`; it runs at once until it first waits.
    pub fn launch(&self, coroutine: impl Future<Output = ()> + 'static) -> Job {
        Job(self.0.launch(Box::pin(coroutine), false, false))
    }

    /// Starts `coroutine` for the value it gives (Kotlin's `async`): await the [`Deferred`] for it.
    pub fn r#async<T: 'static>(&self, coroutine: impl Future<Output = T> + 'static) -> Deferred<T> {
        let value: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
        let slot = value.clone();
        let job = self.0.launch(
            Box::pin(async move {
                let v = coroutine.await;
                *slot.borrow_mut() = Some(v);
            }),
            true,
            false,
        );
        Deferred { job: Job(job), value }
    }

    /// Cancels everything launched here; nothing launched afterwards runs.
    pub fn cancel(&self) {
        self.0.cancel(None);
    }

    pub fn is_active(&self) -> bool {
        !self.0.cancelled.get()
    }

    /// How many of its coroutines are running.
    pub fn active_jobs(&self) -> usize {
        self.0.jobs.borrow().len()
    }

    pub fn dispatcher(&self) -> Dispatcher {
        Dispatcher(self.0.dispatcher.upgrade().expect("kompoz: a scope whose dispatcher is gone"))
    }
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
    /// Running: not completed nor cancelled.
    pub fn is_active(&self) -> bool {
        !self.0.done.get() && !self.0.cancelled.get()
    }
    /// Ended, however (cancelled too).
    pub fn is_completed(&self) -> bool {
        self.0.done.get()
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.get()
    }
    /// Waits until it ends (completed, failed or cancelled).
    pub fn join(&self) -> impl Future<Output = ()> + 'static {
        let job = self.0.clone();
        std::future::poll_fn(move |cx| {
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
