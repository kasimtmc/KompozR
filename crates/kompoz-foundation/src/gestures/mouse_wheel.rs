//! The mouse wheel and a trackpad's pans on a scrollable, as Compose's MouseWheelScrollingLogic.kt,
//! TrackpadScrollingLogic.kt, NonTouchScrollingLogic.kt and DifferentialVelocityTracker.kt, with the scroll
//! configuration of Compose for desktop (DesktopScrollable.desktop.kt): a wheel's notch is 64 dp (Android's scroll
//! factor, what the C++ Kompoz used); smooth: the first 6 dp at once, the rest at 1 dp a millisecond (at most
//! 100 ms), notches meanwhile adding up; a precise wheel (a touchpad's or a free-spinning wheel's parts of
//! notches) at once; the scroll counts as under way until no event came for 50 ms.

use super::scrollable::{NestedScrollBlock, NestedScrollScope, ScrollingLogic};
use kompoz_animation::{
    AnimationScope, AnimationSpec, AnimationState, AnimationVector1D, LinearEasing, MutatePriority, TweenSpec,
    float_animation_state, tween,
};
use kompoz_coroutines::{
    Channel, CoroutineScope, Job, coroutine_scope, supervisor_scope, with_frame_nanos, with_timeout_or_null,
};
use kompoz_graphics::{Density, Dp, IntSize, Offset};
use kompoz_runtime::{KompozitionLocal, named_args};
use kompoz_ui::NodeCx;
use kompoz_ui::input::{NestedScrollSource, PointerEvent, PointerEventPass, PointerEventType, VelocityTracker1D};
use kompoz_ui::platform::{LocalViewConfiguration, WheelScrollAmount};
use kompoz_ui::unit::Velocity;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// The step a smooth wheel scroll makes at once (Compose's AnimationThreshold: AnimationSpeed ×
/// MaxAnimationDuration / a 60 Hz frame).
const AnimationThreshold: Dp = Dp(6.0);
/// How fast the rest goes, dp a millisecond (Compose's AnimationSpeed).
const AnimationSpeed: Dp = Dp(1.0);
/// The longest a step's animation takes, ms (Compose's MaxAnimationDuration).
const MaxAnimationDuration: i32 = 100;
/// How long after its last event a wheel's scroll ends, ms (Compose's ScrollProgressTimeout).
const ScrollProgressTimeout: i64 = 50;

/// What a wheel's notch scrolls (Android's default scroll factor, `ViewConfiguration.getScaledVerticalScrollFactor`
/// at density 1; Compose's AndroidConfig's fallback).
const WheelNotch: Dp = Dp(64.0);

// ---------------------------------------------------------------------------------------------------------------
// The configuration

/// How the mouse wheel scrolls (Compose's ScrollConfig, internal there; each platform's own): whether its steps
/// animate, whether an event is a precise wheel's (scrolled at once), and the scroll an event makes, px (a
/// finger's direction: the wheel down scrolls the content up).
pub trait ScrollConfig {
    /// Whether wheel steps animate (Compose's `isSmoothScrollingEnabled`).
    fn is_smooth_scrolling_enabled(&self) -> bool {
        true
    }

    /// Whether `event` is a precise wheel's (Compose's `isPreciseWheelScroll`).
    fn is_precise_wheel_scroll(&self, event: &PointerEvent) -> bool {
        let _ = event;
        false
    }

    /// The scroll `event` makes in a scrollable of `bounds` (Compose's `calculateMouseWheelScroll`).
    fn calculate_mouse_wheel_scroll(&self, density: Density, event: &PointerEvent, bounds: IntSize) -> Offset;

    /// This configuration as the system says a notch scrolls (KompozR's: [`ViewConfiguration::wheel_scroll_amount`],
    /// Windows' "lines to scroll at a time"); an app's own configuration keeps its own (itself, by default).
    ///
    /// [`ViewConfiguration::wheel_scroll_amount`]: kompoz_ui::platform::ViewConfiguration::wheel_scroll_amount
    fn with_wheel_scroll_amount(self: Rc<Self>, amount: WheelScrollAmount) -> Rc<dyn ScrollConfig>;
}

/// KompozR's wheel (the C++ Kompoz's, Compose for desktop's behaviour): a notch scrolls 64 dp, each axis (Android's
/// scroll factor, as its AndroidConfig does) — as many lines' share of it as the system's notch scrolls (three
/// lines are 64 dp), or the scrollable's size where a notch scrolls a page (Windows' settings); a precise wheel's
/// parts of notches scroll at once (Compose for desktop's heuristic: not a whole number of notches); smooth unless
/// turned off.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DefaultScrollConfig {
    pub is_smooth_scrolling_enabled: bool,
    /// What a notch scrolls, as the system says.
    pub wheel_scroll_amount: WheelScrollAmount,
}

impl Default for DefaultScrollConfig {
    fn default() -> Self {
        DefaultScrollConfig { is_smooth_scrolling_enabled: true, wheel_scroll_amount: WheelScrollAmount::default() }
    }
}

/// An event's notches, both wheels (Compose's `totalScrollDelta`).
fn total_scroll_delta(event: &PointerEvent) -> Offset {
    event.changes.iter().fold(Offset::Zero, |acc, c| acc + c.scroll_delta)
}

impl ScrollConfig for DefaultScrollConfig {
    fn is_smooth_scrolling_enabled(&self) -> bool {
        self.is_smooth_scrolling_enabled
    }

    fn is_precise_wheel_scroll(&self, event: &PointerEvent) -> bool {
        // (Compose for desktop: the precise rotation more than 0.001 from the whole notches)
        let d = total_scroll_delta(event);
        (d.x - d.x.trunc()).abs() > 0.001 || (d.y - d.y.trunc()).abs() > 0.001
    }

    fn calculate_mouse_wheel_scroll(&self, density: Density, event: &PointerEvent, bounds: IntSize) -> Offset {
        let d = total_scroll_delta(event);
        match self.wheel_scroll_amount {
            WheelScrollAmount::Lines(lines) => {
                let factor = -density.to_px(WheelNotch) * lines / WheelScrollAmount::DEFAULT_LINES;
                Offset::new(d.x * factor, d.y * factor)
            }
            WheelScrollAmount::Page => Offset::new(-d.x * bounds.width as f32, -d.y * bounds.height as f32),
        }
    }

    fn with_wheel_scroll_amount(self: Rc<Self>, amount: WheelScrollAmount) -> Rc<dyn ScrollConfig> {
        if self.wheel_scroll_amount == amount {
            return self;
        }
        Rc::new(DefaultScrollConfig { wheel_scroll_amount: amount, ..*self })
    }
}

/// The wheel's configuration where a scrollable is (Compose for desktop's internal LocalScrollConfig).
pub static LocalScrollConfig: KompozitionLocal<Rc<dyn ScrollConfig>> =
    KompozitionLocal::new(|| Rc::new(DefaultScrollConfig::default()));

/// Compose's `platformScrollConfig`: the local one, as the system says a notch scrolls.
pub(crate) fn platform_scroll_config(cx: &NodeCx) -> Rc<dyn ScrollConfig> {
    let amount = cx.current_value_of(&LocalViewConfiguration).wheel_scroll_amount;
    cx.current_value_of(&LocalScrollConfig).with_wheel_scroll_amount(amount)
}

// ---------------------------------------------------------------------------------------------------------------
// What both share

/// Velocities of deltas, each axis apart (Compose's DifferentialVelocityTracker).
struct DifferentialVelocityTracker {
    x: VelocityTracker1D,
    y: VelocityTracker1D,
}

impl DifferentialVelocityTracker {
    fn new() -> DifferentialVelocityTracker {
        DifferentialVelocityTracker { x: VelocityTracker1D::new(true), y: VelocityTracker1D::new(true) }
    }

    fn add_delta(&mut self, time_millis: i64, delta: Offset) {
        self.x.add_data_point(time_millis, delta.x);
        self.y.add_data_point(time_millis, delta.y);
    }

    fn calculate_velocity(&self) -> Velocity {
        Velocity::new(self.x.calculate_velocity_max(f32::MAX), self.y.calculate_velocity_max(f32::MAX))
    }
}

/// What the mouse wheel's and the trackpad's logic share (Compose's NonTouchScrollingLogic).
pub(crate) trait NonTouchScrollingLogic {
    fn update_density(&self, density: Density);
    fn on_pointer_event(&self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize);
    fn start_receiving_events(&self, scope: &CoroutineScope);
}

struct NonTouchBase {
    scrolling_logic: Rc<ScrollingLogic>,
    on_scroll_stopped: Rc<dyn Fn(Velocity)>,
    density: Cell<Density>,
    is_scrolling: Cell<bool>,
    velocity_tracker: RefCell<DifferentialVelocityTracker>,
}

impl NonTouchBase {
    fn new(scrolling_logic: Rc<ScrollingLogic>, on_scroll_stopped: Rc<dyn Fn(Velocity)>, density: Density) -> Self {
        NonTouchBase {
            scrolling_logic,
            on_scroll_stopped,
            density: Cell::new(density),
            is_scrolling: Cell::new(false),
            velocity_tracker: RefCell::new(DifferentialVelocityTracker::new()),
        }
    }

    /// Compose's `userScroll`: a user's mutation (its cancellation by a higher priority one goes on up).
    async fn user_scroll(&self, block: NestedScrollBlock) {
        self.is_scrolling.set(true);
        let scroll = self.scrolling_logic.scroll(MutatePriority::UserInput, block);
        supervisor_scope(move |_| scroll).await;
        self.is_scrolling.set(false);
    }
}

/// Whether any change is consumed (Compose's `PointerEvent.isConsumed`).
fn is_consumed(event: &PointerEvent) -> bool {
    event.changes.iter().any(|c| c.is_consumed())
}

fn consume(event: &PointerEvent) {
    for c in &event.changes {
        c.consume();
    }
}

/// Too little to show (or NaN: Compose's `isLowScrollingDelta`).
fn is_low_scrolling_delta(v: f32) -> bool {
    v.is_nan() || v.abs() < 0.5
}

/// Receives from `channel` while asking for frames, so waiting for it isn't idle (Compose's `busyReceive`: tests
/// trying another scroll after a wheel's wait for its timeout).
async fn busy_receive<T: Send + 'static>(channel: Channel<T>) -> Option<T> {
    coroutine_scope(move |scope| async move {
        let job = scope.launch(async {
            loop {
                with_frame_nanos(|_| ()).await;
            }
        });
        /// Compose's `finally { job.cancel() }`.
        struct CancelJob(Job);
        impl Drop for CancelJob {
            fn drop(&mut self) {
                self.0.cancel();
            }
        }
        let _cancel = CancelJob(job);
        channel.receive().await
    })
    .await
}

// ---------------------------------------------------------------------------------------------------------------
// The mouse wheel

/// A wheel event's scroll (Compose's MouseWheelScrollDelta): px, when, whether it goes at once.
#[derive(Clone, Copy, Debug)]
struct MouseWheelScrollDelta {
    value: Offset,
    time_millis: i64,
    should_apply_immediately: bool,
}

impl std::ops::Add for MouseWheelScrollDelta {
    type Output = MouseWheelScrollDelta;
    fn add(self, other: MouseWheelScrollDelta) -> MouseWheelScrollDelta {
        MouseWheelScrollDelta {
            value: self.value + other.value,
            // (the last one's time)
            time_millis: self.time_millis.max(other.time_millis),
            // (the other's left aside: a false precise wheel while animating)
            should_apply_immediately: self.should_apply_immediately,
        }
    }
}

/// A scrollable's mouse wheel (Compose's MouseWheelScrollingLogic): its events taken in the Main pass (in the
/// Initial one while it scrolls, so it keeps them), sent to a coroutine that scrolls them smoothly in a user's
/// mutation.
pub(crate) struct MouseWheelScrollingLogic {
    base: NonTouchBase,
    mouse_wheel_scroll_config: Rc<dyn ScrollConfig>,
    channel: Channel<MouseWheelScrollDelta>,
    receiving_mouse_wheel_events_job: RefCell<Option<Job>>,
    me: std::rc::Weak<MouseWheelScrollingLogic>,
}

impl MouseWheelScrollingLogic {
    pub(crate) fn new(
        scrolling_logic: Rc<ScrollingLogic>,
        mouse_wheel_scroll_config: Rc<dyn ScrollConfig>,
        on_scroll_stopped: Rc<dyn Fn(Velocity)>,
        density: Density,
    ) -> Rc<MouseWheelScrollingLogic> {
        Rc::new_cyclic(|me| MouseWheelScrollingLogic {
            base: NonTouchBase::new(scrolling_logic, on_scroll_stopped, density),
            mouse_wheel_scroll_config,
            channel: Channel::unlimited(),
            receiving_mouse_wheel_events_job: RefCell::new(None),
            me: me.clone(),
        })
    }

    /// Compose's `onMouseWheel`: the event sent to the coroutine if the scrollable can take it; whether it took it.
    fn on_mouse_wheel(&self, event: &PointerEvent, bounds: IntSize) -> bool {
        let scroll_delta =
            self.mouse_wheel_scroll_config.calculate_mouse_wheel_scroll(self.base.density.get(), event, bounds);
        if self.can_consume_delta(scroll_delta) {
            let should_apply_immediately = !self.mouse_wheel_scroll_config.is_smooth_scrolling_enabled()
                // (a free-spinning wheel or a trackpad: at once, no delay)
                || self.mouse_wheel_scroll_config.is_precise_wheel_scroll(event);
            self.channel
                .try_send(MouseWheelScrollDelta {
                    value: scroll_delta,
                    time_millis: event.changes[0].uptime_millis,
                    should_apply_immediately,
                })
                .is_success()
        } else {
            self.base.is_scrolling.get()
        }
    }

    /// What is waiting, added up (Compose's `sumOrNull`).
    fn sum_or_null(&self) -> Option<MouseWheelScrollDelta> {
        let mut sum: Option<MouseWheelScrollDelta> = None;
        while let Some(i) = self.channel.try_receive().get_or_null() {
            sum = Some(match sum {
                None => i,
                Some(s) => s + i,
            });
        }
        sum
    }

    /// Whether the scrollable can take a delta: along its axis by its angle, and not past its end (Compose's
    /// `canConsumeDelta`).
    fn can_consume_delta(&self, scroll_delta: Offset) -> bool {
        let logic = &self.base.scrolling_logic;
        let delta = logic.to_single_axis_delta_from_angle(logic.reverse_offset_if_needed(scroll_delta));
        let state = logic.scrollable_state.borrow().clone();
        if delta == 0.0 {
            false // another axis'
        } else if delta > 0.0 {
            state.can_scroll_forward()
        } else {
            state.can_scroll_backward()
        }
    }

    fn track_velocity(&self, scroll_delta: MouseWheelScrollDelta) {
        self.base.velocity_tracker.borrow_mut().add_delta(scroll_delta.time_millis, scroll_delta.value);
    }

    /// Compose's `dispatchMouseWheelScroll`: the step, smoothly, then each one coming meanwhile on top of it,
    /// until none comes for the timeout; then the scrollable is told it stopped with the velocity it had.
    async fn dispatch_mouse_wheel_scroll(
        self: Rc<Self>,
        scroll_delta: MouseWheelScrollDelta,
        threshold: f32,
        speed: f32,
    ) {
        let logic = self.base.scrolling_logic.clone();
        let target_scroll_delta = Rc::new(Cell::new(scroll_delta));
        self.track_velocity(scroll_delta);
        // All that waits, added up: no restart for each.
        if let Some(sum) = self.sum_or_null() {
            self.track_velocity(sum);
            target_scroll_delta.set(target_scroll_delta.get() + sum);
        }
        let target_value =
            Rc::new(Cell::new(logic.offset_to_float(logic.reverse_offset_if_needed(target_scroll_delta.get().value))));
        if is_low_scrolling_delta(target_value.get()) {
            return;
        }
        let animation_state = Rc::new(RefCell::new(float_animation_state(0.0)));
        let (me, tsd, tv, state) =
            (self.clone(), target_scroll_delta.clone(), target_value.clone(), animation_state.clone());
        self.base
            .user_scroll(Box::new(move |scope| {
                Box::pin(async move {
                    let mut required_animation = true;
                    // (each turn says whether another is needed)
                    while required_animation {
                        let target_value_leftover = tv.get() - state.borrow().value();
                        if tsd.get().should_apply_immediately || target_value_leftover.abs() < threshold {
                            me.dispatch_scroll(&*scope, target_value_leftover);
                            required_animation =
                                me.wait_next_scroll_delta(ScrollProgressTimeout, &tsd, &tv, &state).await;
                        } else {
                            // The animation starts at the next frame: the threshold at once, no delay.
                            let instant_delta = target_value_leftover.signum() * threshold;
                            me.dispatch_scroll(&*scope, instant_delta);
                            let next = {
                                let s = state.borrow();
                                named_args!(s.copy_float(value = s.value() + instant_delta))
                            };
                            *state.borrow_mut() = next;
                            let duration_millis = ((tv.get() - state.borrow().value()).abs() / speed)
                                .round()
                                .min(MaxAnimationDuration as f32)
                                as i32;
                            let flag = Rc::new(Cell::new(false));
                            let (me2, tsd2, tv2, f) = (me.clone(), tsd.clone(), tv.clone(), flag.clone());
                            let current = state.borrow().clone();
                            me.animate_mouse_wheel_scroll(
                                scope.clone(),
                                current,
                                tv.get(),
                                duration_millis,
                                move |last_value| {
                                    // All that waits, added up: no restart for each.
                                    let next_scroll_delta = me2.sum_or_null();
                                    if let Some(next) = next_scroll_delta {
                                        me2.track_velocity(next);
                                        tsd2.set(tsd2.get() + next);
                                        let l = &me2.base.scrolling_logic;
                                        tv2.set(l.to_single_axis_delta_from_angle(
                                            l.reverse_offset_if_needed(tsd2.get().value),
                                        ));
                                        f.set(!is_low_scrolling_delta(tv2.get() - last_value));
                                    }
                                    next_scroll_delta.is_some()
                                },
                            )
                            .await;
                            required_animation = flag.get();
                            if !required_animation {
                                // Done: the next event waited for before the scroll stops counting as under way.
                                required_animation = me
                                    .wait_next_scroll_delta(
                                        ScrollProgressTimeout - duration_millis as i64,
                                        &tsd,
                                        &tv,
                                        &state,
                                    )
                                    .await;
                            }
                        }
                    }
                })
            }))
            .await;
        let mut velocity = self.base.velocity_tracker.borrow().calculate_velocity();
        if velocity == Velocity::Zero {
            // One event only: the animation's speed, the delta's way.
            let velocity_px_in_ms = (target_value.get().abs() / MaxAnimationDuration as f32).min(speed);
            velocity = logic.float_to_velocity(
                logic.reverse_float_if_needed(target_value.get().signum()) * velocity_px_in_ms * 1000.0,
            );
        }
        (self.base.on_scroll_stopped)(velocity);
    }

    /// The next event within `timeout_millis`: whether it has something to scroll (Compose's `waitNextScrollDelta`).
    async fn wait_next_scroll_delta(
        &self,
        timeout_millis: i64,
        target_scroll_delta: &Rc<Cell<MouseWheelScrollDelta>>,
        target_value: &Rc<Cell<f32>>,
        animation_state: &Rc<RefCell<AnimationState<f32, AnimationVector1D>>>,
    ) -> bool {
        if timeout_millis < 0 {
            return false;
        }
        let channel = self.channel.clone();
        let received =
            with_timeout_or_null(Duration::from_millis(timeout_millis as u64), move |_| busy_receive(channel)).await;
        let Some(Some(it)) = received else { return false };
        // (whether it goes at once stays as it was during the animation: a precise value may come close to a
        // regular one)
        let previous_delta_should_apply_immediately = target_scroll_delta.get().should_apply_immediately;
        target_scroll_delta
            .set(MouseWheelScrollDelta { should_apply_immediately: previous_delta_should_apply_immediately, ..it });
        let logic = &self.base.scrolling_logic;
        target_value.set(
            logic.to_single_axis_delta_from_angle(logic.reverse_offset_if_needed(target_scroll_delta.get().value)),
        );
        // (the last animation's leftover reset)
        *animation_state.borrow_mut() = float_animation_state(0.0);
        self.track_velocity(it);
        !is_low_scrolling_delta(target_value.get())
    }

    /// Compose's `animateMouseWheelScroll`: a linear tween towards the target, stopped where a step isn't taken
    /// or `should_cancel_animation` says (new events).
    async fn animate_mouse_wheel_scroll(
        &self,
        scope: Rc<dyn NestedScrollScope>,
        animation_state: AnimationState<f32, AnimationVector1D>,
        target_value: f32,
        duration_millis: i32,
        should_cancel_animation: impl Fn(f32) -> bool + 'static,
    ) {
        let last_value = Rc::new(Cell::new(animation_state.value()));
        let me = self.me.upgrade().expect("its logic");
        let tween_spec: TweenSpec<f32> = named_args!(tween(duration_millis, easing = Arc::new(LinearEasing)));
        let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(tween_spec);
        let block: Box<dyn for<'a> FnMut(&'a AnimationScope<f32, AnimationVector1D>)> =
            Box::new(move |s: &AnimationScope<f32, AnimationVector1D>| {
                let delta = s.value() - last_value.get();
                if !is_low_scrolling_delta(delta) {
                    let consumed_delta = me.dispatch_scroll(&*scope, delta);
                    if !is_low_scrolling_delta(delta - consumed_delta) {
                        s.cancel_animation();
                        return;
                    }
                    last_value.set(last_value.get() + delta);
                }
                if should_cancel_animation(last_value.get()) {
                    s.cancel_animation();
                }
            });
        named_args!(animation_state.animate_to(target_value, spec, true, block)).await;
    }

    /// A step through the nested scroll chain, no overscroll (Compose's NestedScrollScope
    /// `dispatchMouseWheelScroll`): what it took.
    fn dispatch_scroll(&self, scope: &dyn NestedScrollScope, delta: f32) -> f32 {
        let logic = &self.base.scrolling_logic;
        let offset = logic.float_to_offset(logic.reverse_float_if_needed(delta));
        let consumed = scope.scroll_by(offset, NestedScrollSource::UserInput);
        logic.offset_to_float(logic.reverse_offset_if_needed(consumed))
    }
}

impl NonTouchScrollingLogic for MouseWheelScrollingLogic {
    fn update_density(&self, density: Density) {
        self.base.density.set(density);
    }

    /// Compose's `onPointerEvent`.
    fn on_pointer_event(&self, event: &PointerEvent, pass: PointerEventPass, bounds: IntSize) {
        if event.r#type != PointerEventType::Scroll || is_consumed(event) {
            return;
        }
        // Scrolling already: taken at once, it keeps priority.
        if pass == PointerEventPass::Initial && self.base.is_scrolling.get() {
            self.on_mouse_wheel(event, bounds);
            consume(event);
        }
        // Not scrolling: taken in the Main pass if it can take it.
        if pass == PointerEventPass::Main && !self.base.is_scrolling.get() && self.on_mouse_wheel(event, bounds) {
            consume(event);
        }
    }

    /// Compose's `startReceivingEvents`.
    fn start_receiving_events(&self, scope: &CoroutineScope) {
        if self.receiving_mouse_wheel_events_job.borrow().is_some() {
            return;
        }
        let me = self.me.upgrade().expect("its logic");
        let job = scope.launch(async move {
            /// Compose's `finally { receivingMouseWheelEventsJob = null }`.
            struct Forget(Rc<MouseWheelScrollingLogic>);
            impl Drop for Forget {
                fn drop(&mut self) {
                    *self.0.receiving_mouse_wheel_events_job.borrow_mut() = None;
                }
            }
            let _forget = Forget(me.clone());
            loop {
                let Some(scroll_delta) = me.channel.receive().await else { return };
                let density = me.base.density.get();
                let threshold = density.to_px(AnimationThreshold);
                let speed = density.to_px(AnimationSpeed);
                me.clone().dispatch_mouse_wheel_scroll(scroll_delta, threshold, speed).await;
            }
        });
        // (set unless it ended already, as a job that completes at once clears it first)
        if job.is_active() {
            *self.receiving_mouse_wheel_events_job.borrow_mut() = Some(job);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The trackpad

/// A trackpad pan's step (Compose's TrackpadScrollDelta).
#[derive(Clone, Copy, Debug)]
struct TrackpadScrollDelta {
    value: Offset,
    time_millis: i64,
    is_end: bool,
}

impl std::ops::Add for TrackpadScrollDelta {
    type Output = TrackpadScrollDelta;
    fn add(self, other: TrackpadScrollDelta) -> TrackpadScrollDelta {
        TrackpadScrollDelta {
            value: self.value + other.value,
            time_millis: self.time_millis.max(other.time_millis),
            is_end: self.is_end || other.is_end,
        }
    }
}

/// A scrollable's trackpad pans (Compose's TrackpadScrollingLogic): the fingers' moves (the content follows them)
/// scrolled at once through the overscroll, a fling when they lift.
pub(crate) struct TrackpadScrollingLogic {
    base: NonTouchBase,
    channel: Channel<TrackpadScrollDelta>,
    receiving_pan_events_job: RefCell<Option<Job>>,
    me: std::rc::Weak<TrackpadScrollingLogic>,
}

impl TrackpadScrollingLogic {
    pub(crate) fn new(
        scrolling_logic: Rc<ScrollingLogic>,
        on_scroll_stopped: Rc<dyn Fn(Velocity)>,
        density: Density,
    ) -> Rc<TrackpadScrollingLogic> {
        Rc::new_cyclic(|me| TrackpadScrollingLogic {
            base: NonTouchBase::new(scrolling_logic, on_scroll_stopped, density),
            channel: Channel::unlimited(),
            receiving_pan_events_job: RefCell::new(None),
            me: me.clone(),
        })
    }

    /// Compose's `onPan`: each historical step and the last sent, the end too; whether it took them.
    fn on_pan(&self, event: &PointerEvent) -> bool {
        let mut sent = false;
        if let Some(change) = event.changes.first() {
            for h in change.historical() {
                let delta = -h.pan_offset;
                if self.can_consume_delta(delta) {
                    sent = self
                        .channel
                        .try_send(TrackpadScrollDelta { value: delta, time_millis: h.uptime_millis, is_end: false })
                        .is_success()
                        || sent;
                }
            }
            let delta = -change.pan_offset;
            let is_pan_end = event.r#type == PointerEventType::PanEnd;
            if self.can_consume_delta(delta) || is_pan_end {
                sent = self
                    .channel
                    .try_send(TrackpadScrollDelta {
                        value: delta,
                        time_millis: change.uptime_millis,
                        is_end: is_pan_end,
                    })
                    .is_success()
                    || sent;
            }
        }
        sent || self.base.is_scrolling.get()
    }

    fn sum_or_null(&self) -> Option<TrackpadScrollDelta> {
        let mut sum: Option<TrackpadScrollDelta> = None;
        while let Some(i) = self.channel.try_receive().get_or_null() {
            sum = Some(match sum {
                None => i,
                Some(s) => s + i,
            });
        }
        sum
    }

    /// Along its axis by its angle (Compose's TrackpadScrollingLogic `canConsumeDelta`).
    fn can_consume_delta(&self, scroll_delta: Offset) -> bool {
        let logic = &self.base.scrolling_logic;
        logic.to_single_axis_delta_from_angle(logic.reverse_offset_if_needed(scroll_delta)) != 0.0
    }

    fn track_velocity(&self, scroll_delta: TrackpadScrollDelta) {
        self.base.velocity_tracker.borrow_mut().add_delta(scroll_delta.time_millis, scroll_delta.value);
    }

    /// Compose's `dispatchTrackpadScroll`: each step until the end, then the fling.
    async fn dispatch_trackpad_scroll(self: Rc<Self>, scroll_delta: TrackpadScrollDelta) {
        let target_scroll_delta = Rc::new(Cell::new(scroll_delta));
        self.track_velocity(scroll_delta);
        if let Some(sum) = self.sum_or_null() {
            self.track_velocity(sum);
            target_scroll_delta.set(target_scroll_delta.get() + sum);
        }
        let (me, tsd) = (self.clone(), target_scroll_delta.clone());
        self.base
            .user_scroll(Box::new(move |scope| {
                Box::pin(async move {
                    let l = me.base.scrolling_logic.clone();
                    me.dispatch_scroll(
                        &*scope,
                        l.to_single_axis_delta_from_angle(l.reverse_offset_if_needed(tsd.get().value)),
                    );
                    while !tsd.get().is_end {
                        let Some(next) = busy_receive(me.channel.clone()).await else { return };
                        tsd.set(next);
                        me.track_velocity(next);
                        if let Some(sum) = me.sum_or_null() {
                            me.track_velocity(sum);
                            tsd.set(tsd.get() + sum);
                        }
                        me.dispatch_scroll(
                            &*scope,
                            l.to_single_axis_delta_from_angle(l.reverse_offset_if_needed(tsd.get().value)),
                        );
                    }
                })
            }))
            .await;
        let velocity = self.base.velocity_tracker.borrow().calculate_velocity();
        (self.base.on_scroll_stopped)(velocity);
    }

    /// A step through the chain and the overscroll (Compose's NestedScrollScope `dispatchTrackpadScroll`).
    fn dispatch_scroll(&self, scope: &dyn NestedScrollScope, delta: f32) -> f32 {
        let logic = &self.base.scrolling_logic;
        let offset = logic.float_to_offset(logic.reverse_float_if_needed(delta));
        let consumed = scope.scroll_by_with_overscroll(offset, NestedScrollSource::UserInput);
        logic.offset_to_float(logic.reverse_offset_if_needed(consumed))
    }
}

impl NonTouchScrollingLogic for TrackpadScrollingLogic {
    fn update_density(&self, density: Density) {
        self.base.density.set(density);
    }

    fn on_pointer_event(&self, event: &PointerEvent, pass: PointerEventPass, _bounds: IntSize) {
        if !matches!(event.r#type, PointerEventType::PanStart | PointerEventType::PanMove | PointerEventType::PanEnd) {
            return;
        }
        if is_consumed(event) {
            return;
        }
        if pass == PointerEventPass::Initial && self.base.is_scrolling.get() {
            self.on_pan(event);
            consume(event);
        }
        if pass == PointerEventPass::Main && !self.base.is_scrolling.get() && self.on_pan(event) {
            consume(event);
        }
    }

    fn start_receiving_events(&self, scope: &CoroutineScope) {
        if self.receiving_pan_events_job.borrow().is_some() {
            return;
        }
        let me = self.me.upgrade().expect("its logic");
        let job = scope.launch(async move {
            struct Forget(Rc<TrackpadScrollingLogic>);
            impl Drop for Forget {
                fn drop(&mut self) {
                    *self.0.receiving_pan_events_job.borrow_mut() = None;
                }
            }
            let _forget = Forget(me.clone());
            loop {
                let Some(delta) = me.channel.receive().await else { return };
                me.clone().dispatch_trackpad_scroll(delta).await;
            }
        });
        if job.is_active() {
            *self.receiving_pan_events_job.borrow_mut() = Some(job);
        }
    }
}
