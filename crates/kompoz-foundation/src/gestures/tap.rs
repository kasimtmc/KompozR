//! Taps, as Compose's `TapGestureDetector.kt`: `detect_tap_gestures` (taps, double taps, long presses, presses
//! with a [`PressGestureScope`]), `await_first_down`, `wait_for_up_or_cancellation`; and Compose for desktop's
//! `detectTapGestures(matcher, keyboardModifiers, …)` (TapGesturesDetector.skiko.kt: the events a
//! [`PointerMatcher`] takes, with the keyboard modifiers asked for), which `detect_tap_gestures` is given a matcher
//! or a keyboard modifiers test.

use super::for_each::await_each_gesture;
use crate::pointer_matcher::PointerMatcher;
use kompoz_coroutines::{CoroutineScope, Job, coroutine_scope};
use kompoz_graphics::{Density, Offset, Size};
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerEvent, PointerEventPass, PointerEventType, PointerInputChange, PointerInputScope,
    PointerKeyboardModifiers, PointerType,
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
/// A test of an event's keyboard modifiers (Compose's `keyboardModifiers: PointerKeyboardModifiers.() -> Boolean`).
pub type KeyboardModifiersTest = Rc<dyn Fn(&PointerKeyboardModifiers) -> bool>;

/// `detect_tap_gestures`'s callbacks, set by name, then awaited (Compose's `detectTapGestures(onDoubleTap = …,
/// onLongPress = …, onPress = …, onTap = …)`). Given a [`matcher`](Self::matcher) or a
/// [`keyboard_modifiers`](Self::keyboard_modifiers) test, it is Compose for desktop's overload
/// `detectTapGestures(matcher = PointerMatcher.Primary, keyboardModifiers = { true }, …)` instead.
pub struct DetectTapGestures {
    scope: PointerInputScope,
    on_double_tap: Option<PositionCallback>,
    on_long_press: Option<PositionCallback>,
    on_press: Option<PressCallback>,
    on_tap: Option<PositionCallback>,
    matcher: Option<PointerMatcher>,
    keyboard_modifiers: Option<KeyboardModifiersTest>,
}

impl DetectTapGestures {
    pub(crate) fn new(scope: PointerInputScope) -> DetectTapGestures {
        DetectTapGestures {
            scope,
            on_double_tap: None,
            on_long_press: None,
            on_press: None,
            on_tap: None,
            matcher: None,
            keyboard_modifiers: None,
        }
    }

    /// The events taken (Compose for desktop's `matcher`, [`PointerMatcher::Primary`] by default): a press and
    /// its release both have to match; a mouse's other buttons pressed meanwhile don't end it.
    pub fn matcher(mut self, matcher: PointerMatcher) -> Self {
        self.matcher = Some(matcher);
        self
    }

    /// What the keyboard's modifiers have to be for a press to count (Compose for desktop's `keyboardModifiers`);
    /// a modifier let go before the release cancels the tap.
    pub fn keyboard_modifiers(
        mut self,
        keyboard_modifiers: impl Fn(&PointerKeyboardModifiers) -> bool + 'static,
    ) -> Self {
        self.keyboard_modifiers = Some(Rc::new(keyboard_modifiers));
        self
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
        let DetectTapGestures { scope, on_double_tap, on_long_press, on_press, on_tap, matcher, keyboard_modifiers } =
            self;
        if matcher.is_some() || keyboard_modifiers.is_some() {
            let gesture = MatchedTapGesture {
                matcher: matcher.unwrap_or_default(),
                keyboard_modifiers: keyboard_modifiers.unwrap_or_else(|| Rc::new(|_: &PointerKeyboardModifiers| true)),
                on_double_tap,
                on_long_press,
                on_press,
                on_tap,
            };
            return Box::pin(gesture.detect(scope));
        }
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
// Compose for desktop's detectTapGestures(matcher, keyboardModifiers, …)

/// Compose desktop's `isAllPressedDown`: a mouse's press (a button pressed, the others may have been already), or
/// every pointer gone down.
pub(crate) fn is_all_pressed_down(event: &PointerEvent, require_unconsumed: bool) -> bool {
    (event.r#type == PointerEventType::Press
        && event.changes.iter().all(|c| c.r#type == PointerType::Mouse && (!require_unconsumed || !c.is_consumed())))
        || event
            .changes
            .iter()
            .all(|c| if require_unconsumed { c.changed_to_down() } else { c.changed_to_down_ignore_consumed() })
}

/// Compose desktop's `isAllPressedUp`: a mouse's release (of a button, others may stay pressed), or every pointer
/// gone up.
pub(crate) fn is_all_pressed_up(event: &PointerEvent, require_unconsumed: bool) -> bool {
    (event.r#type == PointerEventType::Release
        && event.changes.iter().all(|c| c.r#type == PointerType::Mouse && (!require_unconsumed || !c.is_consumed())))
        || event
            .changes
            .iter()
            .all(|c| if require_unconsumed { c.changed_to_up() } else { c.changed_to_up_ignore_consumed() })
}

/// Compose desktop's `awaitPress`: the next press `filter` takes.
pub(crate) async fn await_press(
    s: &AwaitPointerEventScope,
    filter: &dyn Fn(&PointerEvent) -> bool,
    require_unconsumed: bool,
) -> PointerEvent {
    loop {
        let event = s.await_pointer_event().await;
        if is_all_pressed_down(&event, require_unconsumed) && filter(&event) {
            return event;
        }
    }
}

/// Compose desktop's `awaitReleaseOrCancelled`: the release `filter` takes; none if a pointer went out of bounds,
/// `cancel_if` says so or a change was consumed (in the final pass). Everything consumed meanwhile when
/// `consume_until_release`.
pub(crate) async fn await_release_or_cancelled(
    s: &AwaitPointerEventScope,
    consume_until_release: bool,
    cancel_if: &dyn Fn(&PointerEvent) -> bool,
    filter: &dyn Fn(&PointerEvent) -> bool,
) -> Option<PointerEvent> {
    loop {
        let event = s.await_pointer_event().await;
        if event.changes.iter().any(|c| c.is_out_of_bounds(s.size(), Size::Zero)) || cancel_if(&event) {
            return None;
        }
        let released = is_all_pressed_up(&event, true) && filter(&event);
        if consume_until_release {
            s.current_event().changes.iter().for_each(|c| c.consume());
        }
        // (cancelled by a consumption: the final pass after the main one looked at)
        let consume_check = s.await_pointer_event_with(PointerEventPass::Final).await;
        if consume_check.changes.iter().any(|c| c.is_consumed()) {
            return None;
        }
        if released {
            return Some(event);
        }
    }
}

/// Compose desktop's `awaitSecondPressUnconsumed`: a second press within the double tap timeout, no sooner than its
/// minimum time after the first's release.
async fn await_second_press_unconsumed(
    s: &AwaitPointerEventScope,
    first_up: &PointerInputChange,
    filter: &dyn Fn(&PointerEvent) -> bool,
) -> Option<PointerEvent> {
    let vc = s.view_configuration();
    let min_uptime = first_up.uptime_millis + vc.double_tap_min_time_millis();
    s.with_timeout_or_null(vc.double_tap_timeout_millis(), |s| async move {
        loop {
            let event = await_press(&s, filter, true).await;
            if event.changes[0].uptime_millis >= min_uptime {
                return event;
            }
        }
    })
    .await
}

/// Compose for desktop's matcher-taking `detectTapGestures`.
struct MatchedTapGesture {
    matcher: PointerMatcher,
    keyboard_modifiers: KeyboardModifiersTest,
    on_double_tap: Option<PositionCallback>,
    on_long_press: Option<PositionCallback>,
    on_press: Option<PressCallback>,
    on_tap: Option<PositionCallback>,
}

/// How a press of a matched tap ended: released, or still held when its long press timeout came, or cancelled.
enum PressEnd {
    Released(PointerEvent),
    TimedOut,
    Cancelled,
}

impl MatchedTapGesture {
    async fn detect(self, scope: PointerInputScope) {
        let density = scope.density();
        let gesture = Rc::new(self);
        coroutine_scope(move |cs| async move {
            let press_scope = PressGestureScope::new(density);
            loop {
                let (gesture, cs, press_scope) = (gesture.clone(), cs.clone(), press_scope.clone());
                scope
                    .await_pointer_event_scope(move |s| async move { gesture.process(&s, &cs, &press_scope).await })
                    .await;
            }
        })
        .await
    }

    fn filter(&self, event: &PointerEvent) -> bool {
        self.matcher.matches(event) && (self.keyboard_modifiers)(&event.keyboard_modifiers)
    }

    /// Launches a press's change once the reset before it is done (Compose launches them in order).
    fn launch_after(cs: &CoroutineScope, before: &Job, block: impl Future<Output = ()> + 'static) -> Job {
        let before = before.clone();
        cs.launch(async move {
            before.join().await;
            block.await;
        })
    }

    fn launch_on_press(&self, cs: &CoroutineScope, press: &PressGestureScope, reset: &Job, at: Offset) {
        if let Some(on_press) = &self.on_press {
            let (on_press, press) = (on_press.clone(), press.clone());
            Self::launch_after(cs, reset, async move { on_press(press, at).await });
        }
    }

    /// Waits for the press's release (within the long press timeout when there is a long press to tell).
    async fn await_press_end(&self, s: &AwaitPointerEventScope) -> PressEnd {
        let wait = |s: AwaitPointerEventScope| async move {
            let cancel_if = |e: &PointerEvent| !(self.keyboard_modifiers)(&e.keyboard_modifiers);
            await_release_or_cancelled(&s, false, &cancel_if, &|e| self.filter(e)).await
        };
        let release = if self.on_long_press.is_none() {
            Some(wait(s.clone()).await)
        } else {
            // (Compose's `withTimeoutOrNull(longPressTimeout) { awaitReleaseOrCancelled(…) }`)
            s.with_timeout_or_null(s.view_configuration().long_press_timeout_millis(), wait).await
        };
        match release {
            None => PressEnd::TimedOut,
            Some(None) => PressEnd::Cancelled,
            Some(Some(r)) => {
                r.changes.iter().for_each(|c| c.consume());
                PressEnd::Released(r)
            }
        }
    }

    /// A long press: told, then everything consumed until the button the matcher takes is let go (whatever the
    /// keyboard's modifiers then).
    async fn long_press(
        &self,
        s: &AwaitPointerEventScope,
        cs: &CoroutineScope,
        press: &PressGestureScope,
        reset: &Job,
        at: Offset,
    ) {
        if let Some(on_long_press) = &self.on_long_press {
            on_long_press(at);
            let matcher = &self.matcher;
            await_release_or_cancelled(s, true, &|_| false, &|e| matcher.matches(e)).await;
            let press = press.clone();
            Self::launch_after(cs, reset, async move { press.release() });
        }
    }

    async fn process(&self, s: &AwaitPointerEventScope, cs: &CoroutineScope, press: &PressGestureScope) {
        let p = press.clone();
        let reset = cs.launch(async move { p.reset().await });
        let down = await_press(s, &|e| self.filter(e), true).await;
        down.changes[0].consume();
        let down_at = down.changes[0].position;
        self.launch_on_press(cs, press, &reset, down_at);
        let first_release = match self.await_press_end(s).await {
            PressEnd::Cancelled => {
                let p = press.clone();
                Self::launch_after(cs, &reset, async move { p.cancel() });
                return;
            }
            PressEnd::TimedOut => {
                self.long_press(s, cs, press, &reset, down_at).await;
                return;
            }
            PressEnd::Released(r) => r,
        };
        let p = press.clone();
        let released = Self::launch_after(cs, &reset, async move { p.release() });
        let first_up_at = first_release.changes[0].position;
        let Some(on_double_tap) = &self.on_double_tap else {
            if let Some(on_tap) = &self.on_tap {
                on_tap(first_up_at);
            }
            return;
        };
        let second_press = await_second_press_unconsumed(s, &first_release.changes[0], &|e| self.filter(e)).await;
        let Some(second_press) = second_press else {
            if let Some(on_tap) = &self.on_tap {
                on_tap(first_up_at);
            }
            return;
        };
        second_press.changes.iter().for_each(|c| c.consume());
        let p = press.clone();
        let reset = Self::launch_after(cs, &released, async move { p.reset().await });
        let second_at = second_press.changes[0].position;
        self.launch_on_press(cs, press, &reset, second_at);
        match self.await_press_end(s).await {
            PressEnd::Cancelled => {
                let p = press.clone();
                Self::launch_after(cs, &reset, async move { p.cancel() });
            }
            PressEnd::TimedOut => self.long_press(s, cs, press, &reset, second_at).await,
            PressEnd::Released(second_release) => {
                let p = press.clone();
                Self::launch_after(cs, &reset, async move { p.release() });
                on_double_tap(second_release.changes[0].position);
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
