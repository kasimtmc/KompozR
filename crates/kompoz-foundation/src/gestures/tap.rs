//! Taps, as Compose's `TapGestureDetector.kt`: `detect_tap_gestures` (taps, double taps, long presses, presses
//! with a [`PressGestureScope`]), `await_first_down`, `wait_for_up_or_cancellation`.

use super::for_each::await_each_gesture;
use kompoz_coroutines::{CoroutineScope, Job, coroutine_scope};
use kompoz_graphics::{Density, Offset};
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerEvent, PointerEventPass, PointerInputChange, PointerInputScope, PointerType,
};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fmt;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Poll, Waker};

/// Whether a first down is only a mouse's primary button (Compose desktop's `firstDownRefersToPrimaryMouseButtonOnly`).
pub(crate) const FIRST_DOWN_REFERS_TO_PRIMARY_MOUSE_BUTTON_ONLY: bool = true;

/// Whether every change of the event went down (and, for a mouse, with its primary button: Compose's
/// `isChangedToDown`).
pub(crate) fn is_changed_to_down(
    event: &PointerEvent,
    require_unconsumed: bool,
    only_primary_mouse_button: bool,
) -> bool {
    let only_primary = only_primary_mouse_button && event.changes.iter().all(|c| c.r#type == PointerType::Mouse);
    if only_primary && !event.buttons.is_primary_pressed() {
        return false;
    }
    event
        .changes
        .iter()
        .all(|c| if require_unconsumed { c.changed_to_down() } else { c.changed_to_down_ignore_consumed() })
}

/// Compose's `awaitFirstDown`.
pub(crate) async fn await_first_down(
    s: &AwaitPointerEventScope,
    require_unconsumed: bool,
    pass: PointerEventPass,
) -> PointerInputChange {
    loop {
        let event = s.await_pointer_event_with(pass).await;
        if is_changed_to_down(&event, require_unconsumed, FIRST_DOWN_REFERS_TO_PRIMARY_MOUSE_BUTTON_ONLY) {
            return event.changes[0].clone();
        }
    }
}

/// Compose's `waitForUpOrCancellation`.
pub(crate) async fn wait_for_up_or_cancellation(
    s: &AwaitPointerEventScope,
    pass: PointerEventPass,
) -> Option<PointerInputChange> {
    loop {
        let event = s.await_pointer_event_with(pass).await;
        if event.changes.iter().all(|c| c.changed_to_up()) {
            return Some(event.changes[0].clone());
        }
        let (size, padding) = (s.size(), s.extended_touch_padding());
        if event.changes.iter().any(|c| c.is_consumed() || c.is_out_of_bounds(size, padding)) {
            return None; // cancelled
        }
        // Consumed in the Final pass (an outer scroller took it, say): cancelled too.
        let check = s.await_pointer_event_with(PointerEventPass::Final).await;
        if check.changes.iter().any(|c| c.is_consumed()) {
            return None;
        }
    }
}

/// How a wait for a long press ended (Compose's LongPressResult).
pub(crate) enum LongPressResult {
    Success,
    Released(PointerInputChange),
    Canceled,
}

/// Compose's `waitForLongPress`.
pub(crate) async fn wait_for_long_press(s: &AwaitPointerEventScope, pass: PointerEventPass) -> LongPressResult {
    let timeout = s.view_configuration().long_press_timeout_millis();
    let waited = s
        .with_timeout(timeout, |s| async move {
            loop {
                let event = s.await_pointer_event_with(pass).await;
                if event.changes.iter().all(|c| c.changed_to_up()) {
                    return LongPressResult::Released(event.changes[0].clone());
                }
                if event.is_deep_press() {
                    return LongPressResult::Success;
                }
                let (size, padding) = (s.size(), s.extended_touch_padding());
                if event.changes.iter().any(|c| c.is_consumed() || c.is_out_of_bounds(size, padding)) {
                    return LongPressResult::Canceled;
                }
                let check = s.await_pointer_event_with(PointerEventPass::Final).await;
                if check.changes.iter().any(|c| c.is_consumed()) {
                    return LongPressResult::Canceled;
                }
            }
        })
        .await;
    waited.unwrap_or(LongPressResult::Success)
}

/// Consumes everything until all pointers are up (Compose's `consumeUntilUp`).
async fn consume_until_up(s: &AwaitPointerEventScope) {
    loop {
        let event = s.await_pointer_event().await;
        event.changes.iter().for_each(|c| c.consume());
        if !event.changes.iter().any(|c| c.pressed) {
            break;
        }
    }
}

/// A second press within the double tap timeout, no sooner than its minimum time (Compose's `awaitSecondDown`).
async fn await_second_down(s: &AwaitPointerEventScope, first_up: &PointerInputChange) -> Option<PointerInputChange> {
    let vc = s.view_configuration();
    let min_uptime = first_up.uptime_millis + vc.double_tap_min_time_millis();
    s.with_timeout_or_null(vc.double_tap_timeout_millis(), |s| async move {
        loop {
            let change = await_first_down(&s, true, PointerEventPass::Main).await;
            if change.uptime_millis >= min_uptime {
                return change;
            }
        }
    })
    .await
}

// ---------------------------------------------------------------------------------------------------------------
// The press scope

/// A press was cancelled (Compose's GestureCancellationException).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GestureCancellation;

impl fmt::Display for GestureCancellation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("The press gesture was canceled.")
    }
}
impl std::error::Error for GestureCancellation {}

/// Kotlin's fair Mutex, as the press scope uses it: a lock handed to who waited first.
#[derive(Default)]
struct Mutex {
    locked: Cell<bool>,
    waiters: RefCell<VecDeque<Rc<LockWaiter>>>,
}

#[derive(Default)]
struct LockWaiter {
    granted: Cell<bool>,
    waker: RefCell<Option<Waker>>,
}

impl Mutex {
    async fn lock(&self) {
        let mut mine: Option<Rc<LockWaiter>> = None;
        /// Gives back a lock it was handed but dropped before taking, or leaves the queue.
        struct Leave<'a>(&'a Mutex, Option<Rc<LockWaiter>>, bool);
        impl Drop for Leave<'_> {
            fn drop(&mut self) {
                if self.2 {
                    return;
                }
                if let Some(w) = self.1.take() {
                    if w.granted.get() {
                        self.0.unlock();
                    } else {
                        self.0.waiters.borrow_mut().retain(|x| !Rc::ptr_eq(x, &w));
                    }
                }
            }
        }
        let mut leave = Leave(self, None, false);
        std::future::poll_fn(|cx| {
            if let Some(w) = &mine {
                if w.granted.get() {
                    return Poll::Ready(());
                }
                *w.waker.borrow_mut() = Some(cx.waker().clone());
                return Poll::Pending;
            }
            if !self.locked.get() {
                self.locked.set(true);
                return Poll::Ready(());
            }
            let w = Rc::new(LockWaiter { granted: Cell::new(false), waker: RefCell::new(Some(cx.waker().clone())) });
            self.waiters.borrow_mut().push_back(w.clone());
            leave.1 = Some(w.clone());
            mine = Some(w);
            Poll::Pending
        })
        .await;
        leave.2 = true;
    }

    fn unlock(&self) {
        let next = self.waiters.borrow_mut().pop_front();
        match next {
            Some(w) => {
                w.granted.set(true);
                let waker = w.waker.borrow_mut().take();
                if let Some(waker) = waker {
                    waker.wake();
                }
            }
            None => self.locked.set(false),
        }
    }
}

struct PressState {
    density: Density,
    is_released: Cell<bool>,
    is_canceled: Cell<bool>,
    mutex: Mutex,
}

/// What an `on_press` callback gets (Compose's PressGestureScope): await the press's release.
#[derive(Clone)]
pub struct PressGestureScope(Rc<PressState>);

impl PressGestureScope {
    fn new(density: Density) -> PressGestureScope {
        PressGestureScope(Rc::new(PressState {
            density,
            is_released: Cell::new(false),
            is_canceled: Cell::new(false),
            mutex: Mutex::default(),
        }))
    }

    /// The layout's density (PressGestureScope is a Density in Compose).
    pub fn density(&self) -> Density {
        self.0.density
    }

    fn cancel(&self) {
        self.0.is_canceled.set(true);
        if self.0.mutex.locked.get() {
            self.0.mutex.unlock();
        }
    }

    fn release(&self) {
        self.0.is_released.set(true);
        if self.0.mutex.locked.get() {
            self.0.mutex.unlock();
        }
    }

    async fn reset(&self) {
        self.0.mutex.lock().await;
        self.0.is_released.set(false);
        self.0.is_canceled.set(false);
    }

    /// Waits for the press to be released; `Err` if it was cancelled instead (Compose's `awaitRelease`, which
    /// throws then).
    pub async fn await_release(&self) -> Result<(), GestureCancellation> {
        if self.try_await_release().await { Ok(()) } else { Err(GestureCancellation) }
    }

    /// Waits for the press to end: whether it was released, not cancelled (Compose's `tryAwaitRelease`).
    pub async fn try_await_release(&self) -> bool {
        if !self.0.is_released.get() && !self.0.is_canceled.get() {
            self.0.mutex.lock().await;
            self.0.mutex.unlock();
        }
        self.0.is_released.get()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// detect_tap_gestures

pub(crate) type PositionCallback = Rc<dyn Fn(Offset)>;
pub(crate) type PressCallback = Rc<dyn Fn(PressGestureScope, Offset) -> Pin<Box<dyn Future<Output = ()>>>>;

/// `detect_tap_gestures`'s callbacks, set by name, then awaited (Compose's `detectTapGestures(onDoubleTap = …,
/// onLongPress = …, onPress = …, onTap = …)`).
pub struct DetectTapGestures {
    scope: PointerInputScope,
    on_double_tap: Option<PositionCallback>,
    on_long_press: Option<PositionCallback>,
    on_press: Option<PressCallback>,
    on_tap: Option<PositionCallback>,
}

impl DetectTapGestures {
    pub(crate) fn new(scope: PointerInputScope) -> DetectTapGestures {
        DetectTapGestures { scope, on_double_tap: None, on_long_press: None, on_press: None, on_tap: None }
    }

    /// A second tap within the double tap timeout (taps then wait for it).
    pub fn on_double_tap(mut self, on_double_tap: impl Fn(Offset) + 'static) -> Self {
        self.on_double_tap = Some(Rc::new(on_double_tap));
        self
    }

    /// A press held for the long press timeout (or a deep press).
    pub fn on_long_press(mut self, on_long_press: impl Fn(Offset) + 'static) -> Self {
        self.on_long_press = Some(Rc::new(on_long_press));
        self
    }

    /// Each press, as it starts; its [`PressGestureScope`] awaits its end.
    pub fn on_press<F, Fut>(mut self, on_press: F) -> Self
    where
        F: Fn(PressGestureScope, Offset) -> Fut + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        self.on_press = Some(Rc::new(move |scope, offset| Box::pin(on_press(scope, offset))));
        self
    }

    /// A tap.
    pub fn on_tap(mut self, on_tap: impl Fn(Offset) + 'static) -> Self {
        self.on_tap = Some(Rc::new(on_tap));
        self
    }
}

impl IntoFuture for DetectTapGestures {
    type Output = ();
    type IntoFuture = Pin<Box<dyn Future<Output = ()>>>;

    fn into_future(self) -> Self::IntoFuture {
        let DetectTapGestures { scope, on_double_tap, on_long_press, on_press, on_tap } = self;
        Box::pin(async move {
            let density = scope.density();
            coroutine_scope(move |cs| async move {
                let press_scope = PressGestureScope::new(density);
                await_each_gesture(&scope, move |s| {
                    let gesture = TapGesture {
                        cs: cs.clone(),
                        press: press_scope.clone(),
                        on_double_tap: on_double_tap.clone(),
                        on_long_press: on_long_press.clone(),
                        on_press: on_press.clone(),
                        on_tap: on_tap.clone(),
                    };
                    async move { gesture.process(&s).await }
                })
                .await
            })
            .await
        })
    }
}

/// One tap gesture's parts (Compose's `processTapGesture` arguments).
struct TapGesture {
    cs: CoroutineScope,
    press: PressGestureScope,
    on_double_tap: Option<PositionCallback>,
    on_long_press: Option<PositionCallback>,
    on_press: Option<PressCallback>,
    on_tap: Option<PositionCallback>,
}

impl TapGesture {
    /// Launches `block` once `reset_job` is done (Compose's `launchAwaitingReset`, undispatched).
    fn launch_awaiting_reset(&self, reset_job: &Job, block: impl Future<Output = ()> + 'static) -> Job {
        let reset_job = reset_job.clone();
        self.cs.launch(async move {
            reset_job.join().await;
            block.await;
        })
    }

    fn launch_on_press(&self, reset_job: &Job, at: Offset) {
        if let Some(on_press) = &self.on_press {
            let (on_press, press) = (on_press.clone(), self.press.clone());
            self.launch_awaiting_reset(reset_job, async move { on_press(press, at).await });
        }
    }

    fn launch_release(&self, reset_job: &Job) -> Job {
        let press = self.press.clone();
        self.launch_awaiting_reset(reset_job, async move { press.release() })
    }

    fn launch_cancel(&self, reset_job: &Job) -> Job {
        let press = self.press.clone();
        self.launch_awaiting_reset(reset_job, async move { press.cancel() })
    }

    fn tap(&self, at: Offset) {
        if let Some(on_tap) = &self.on_tap {
            on_tap(at);
        }
    }

    /// Compose's `processTapGesture`.
    async fn process(&self, s: &AwaitPointerEventScope) {
        let down = await_first_down(s, true, PointerEventPass::Main).await;
        down.consume();
        let press = self.press.clone();
        let mut reset_job = self.cs.launch(async move { press.reset().await });
        self.launch_on_press(&reset_job, down.position);
        let up_or_cancel = match &self.on_long_press {
            None => wait_for_up_or_cancellation(s, PointerEventPass::Main).await,
            Some(on_long_press) => match wait_for_long_press(s, PointerEventPass::Main).await {
                LongPressResult::Success => {
                    on_long_press(down.position);
                    consume_until_up(s).await;
                    self.launch_release(&reset_job);
                    return;
                }
                LongPressResult::Released(up) => Some(up),
                LongPressResult::Canceled => None,
            },
        };
        let cancel_or_release_job = match &up_or_cancel {
            None => self.launch_cancel(&reset_job),
            Some(up) => {
                up.consume();
                self.launch_release(&reset_job)
            }
        };
        let Some(up) = up_or_cancel else { return };
        let Some(on_double_tap) = &self.on_double_tap else {
            self.tap(up.position); // no need to wait for a double tap
            return;
        };
        let Some(second_down) = await_second_down(s, &up).await else {
            self.tap(up.position); // no second tap came
            return;
        };
        let press = self.press.clone();
        reset_job = self.cs.launch(async move {
            cancel_or_release_job.join().await;
            press.reset().await;
        });
        self.launch_on_press(&reset_job, second_down.position);
        let second_up = match &self.on_long_press {
            None => wait_for_up_or_cancellation(s, PointerEventPass::Main).await,
            Some(on_long_press) => match wait_for_long_press(s, PointerEventPass::Main).await {
                LongPressResult::Success => {
                    on_long_press(second_down.position);
                    consume_until_up(s).await;
                    self.launch_release(&reset_job);
                    return;
                }
                LongPressResult::Released(up) => Some(up),
                LongPressResult::Canceled => None,
            },
        };
        match second_up {
            Some(second_up) => {
                second_up.consume();
                self.launch_release(&reset_job);
                on_double_tap(second_up.position);
            }
            None => {
                self.launch_cancel(&reset_job);
                self.tap(up.position);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// detect_tap_and_press

/// Taps and presses without waiting for a double tap or a long press (Compose's internal `detectTapAndPress`, a
/// text field's): each down is consumed and told to `on_press` (with the scope awaiting its release), each up to
/// `on_tap`.
pub(crate) async fn detect_tap_and_press(
    scope: PointerInputScope,
    on_press: Option<PressCallback>,
    on_tap: Option<PositionCallback>,
) {
    let density = scope.density();
    coroutine_scope(move |cs| async move {
        let press_scope = PressGestureScope::new(density);
        await_each_gesture(&scope, move |s| {
            let gesture = TapGesture {
                cs: cs.clone(),
                press: press_scope.clone(),
                on_double_tap: None,
                on_long_press: None,
                on_press: on_press.clone(),
                on_tap: on_tap.clone(),
            };
            async move {
                let press = gesture.press.clone();
                let reset_job = gesture.cs.launch(async move { press.reset().await });
                let down = await_first_down(&s, true, PointerEventPass::Main).await;
                down.consume();
                gesture.launch_on_press(&reset_job, down.position);
                match wait_for_up_or_cancellation(&s, PointerEventPass::Main).await {
                    None => {
                        gesture.launch_cancel(&reset_job);
                    }
                    Some(up) => {
                        up.consume();
                        gesture.launch_release(&reset_job);
                        gesture.tap(up.position);
                    }
                }
            }
        })
        .await
    })
    .await
}

/// Reports each down, unconsumed or not, and the up of that pointer when `on_up` is given (Compose's internal
/// `detectPressDownGesture`, the selection handles').
pub(crate) async fn detect_press_down_gesture(
    scope: PointerInputScope,
    on_down: Rc<dyn Fn(Offset)>,
    on_up: Option<Rc<dyn Fn()>>,
) {
    await_each_gesture(&scope, move |s| {
        let (on_down, on_up) = (on_down.clone(), on_up.clone());
        async move {
            let down = await_first_down(&s, false, PointerEventPass::Main).await;
            on_down(down.position);
            if let Some(on_up) = on_up {
                loop {
                    let event = s.await_pointer_event().await;
                    if !event.changes.iter().any(|c| c.id == down.id && c.pressed) {
                        break;
                    }
                }
                on_up();
            }
        }
    })
    .await
}
