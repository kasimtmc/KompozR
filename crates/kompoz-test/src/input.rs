//! Input injection, as Compose's ui-test `TouchInjectionScope`, `MouseInjectionScope` and `InputDispatcher`:
//! `perform_touch_input { … }` and `perform_mouse_input { … }` on a node queue events at positions local to it
//! (each `move` 16 ms after the last, unless told otherwise), then send them to the host's input entry points at
//! their times, the rule's virtual clock run up to each (frames, delays and timeouts due run first). What a gesture
//! leaves (pointers still down, the cursor) carries over to the next `perform_*`.

use crate::{FRAME_NANOS, KompozTestRule, SemanticsNodeInteraction};
use kompoz_graphics::{Density, IntSize, Offset, Rect, lerp_offset};
use kompoz_runtime::named;
use kompoz_ui::input::{PointerButton, PointerButtons, PointerType};
use kompoz_ui::platform::{LocalViewConfiguration, ViewConfiguration};

/// The time between two events of a gesture, ms (Compose's `InputDispatcher.eventPeriodMillis`).
pub const EVENT_PERIOD_MILLIS: i64 = 16;

/// The host pointer id the mouse is injected with.
const MOUSE_POINTER_ID: i64 = -1;

/// Between two times, rounded (Compose's `lerp(Long, Long, Float)`).
fn lerp_time(start: i64, stop: i64, fraction: f32) -> i64 {
    start + ((stop - start) as f64 * fraction as f64).round() as i64
}

/// An event waiting to be sent, at its time (ms, the rule's clock), positions in the root.
#[derive(Clone, Debug)]
enum InjectedEvent {
    TouchDown { time: i64, pointer_id: i64, position: Offset },
    TouchMove { time: i64, positions: Vec<(i64, Offset)> },
    TouchUp { time: i64, pointer_id: i64, position: Offset },
    TouchCancel { time: i64 },
    MouseMove { time: i64, position: Offset },
    MouseButton { time: i64, position: Offset, button: PointerButton, pressed: bool },
    MouseCancel { time: i64 },
    MouseExit { time: i64, position: Offset },
    MouseScroll { time: i64, position: Offset, delta: Offset },
}

/// What injected input leaves between `perform_*` calls (Compose's InputDispatcherState): the fingers of a gesture
/// under way, the cursor and its buttons, the last event's time.
#[derive(Default)]
pub(crate) struct InjectionState {
    touches: Vec<(i32, Offset)>,
    cursor: Offset,
    cursor_in: bool,
    buttons: PointerButtons,
    last_event_time: i64,
}

/// What both injection scopes have (Compose's InjectionScope): the node's size and the positions on it (local to
/// it), the gesture thresholds, the event clock.
pub trait InjectionScope {
    /// The time between two events, ms.
    fn event_period_millis(&self) -> i64 {
        EVENT_PERIOD_MILLIS
    }
    /// Moves the event clock on (the next event comes that much later).
    fn advance_event_time(&mut self, duration_millis: i64);
    /// The node's size, px (its bounds in the root, cut by clips).
    fn visible_size(&self) -> IntSize;
    /// The gesture thresholds where the node was composed.
    fn view_configuration(&self) -> ViewConfiguration;
    fn density(&self) -> Density;

    fn width(&self) -> i32 {
        self.visible_size().width
    }
    fn height(&self) -> i32 {
        self.visible_size().height
    }
    fn left(&self) -> f32 {
        0.0
    }
    fn top(&self) -> f32 {
        0.0
    }
    fn center_x(&self) -> f32 {
        self.width() as f32 / 2.0
    }
    fn center_y(&self) -> f32 {
        self.height() as f32 / 2.0
    }
    /// The last pixel's x (Compose's `right`: width - 1).
    fn right(&self) -> f32 {
        if self.width() == 0 { 0.0 } else { self.width() as f32 - 1.0 }
    }
    fn bottom(&self) -> f32 {
        if self.height() == 0 { 0.0 } else { self.height() as f32 - 1.0 }
    }
    fn top_left(&self) -> Offset {
        Offset::new(self.left(), self.top())
    }
    fn top_center(&self) -> Offset {
        Offset::new(self.center_x(), self.top())
    }
    fn top_right(&self) -> Offset {
        Offset::new(self.right(), self.top())
    }
    fn center_left(&self) -> Offset {
        Offset::new(self.left(), self.center_y())
    }
    fn center(&self) -> Offset {
        Offset::new(self.center_x(), self.center_y())
    }
    fn center_right(&self) -> Offset {
        Offset::new(self.right(), self.center_y())
    }
    fn bottom_left(&self) -> Offset {
        Offset::new(self.left(), self.bottom())
    }
    fn bottom_center(&self) -> Offset {
        Offset::new(self.center_x(), self.bottom())
    }
    fn bottom_right(&self) -> Offset {
        Offset::new(self.right(), self.bottom())
    }
    /// A fraction of the size from the top left (Compose's `percentOffset`).
    fn percent_offset(&self, x: f32, y: f32) -> Offset {
        Offset::new(x * self.width() as f32, y * self.height() as f32)
    }
}

/// What both scopes share: where the node is, its thresholds, the clock, the queue.
struct ScopeBase {
    bounds: Rect,
    view_configuration: ViewConfiguration,
    density: Density,
    root_size: IntSize,
    current_time: i64,
    events: Vec<InjectedEvent>,
}

impl ScopeBase {
    fn new(interaction: &SemanticsNodeInteraction<'_>) -> (ScopeBase, InjectionState) {
        let node = interaction.fetch_semantics_node();
        let rule = interaction.rule;
        let state = std::mem::take(&mut *rule.injection.borrow_mut());
        let clock_millis = rule.clock.get() / 1_000_000;
        let base = ScopeBase {
            bounds: node.bounds_in_root(),
            view_configuration: node.layout_node().current_value_of(&LocalViewConfiguration),
            density: rule.density(),
            root_size: rule.host().viewport_size(),
            current_time: clock_millis.max(state.last_event_time),
            events: Vec::new(),
        };
        (base, state)
    }

    fn local_to_root(&self, position: Offset) -> Offset {
        position + self.bounds.top_left()
    }

    fn root_to_local(&self, position: Offset) -> Offset {
        position - self.bounds.top_left()
    }

    fn visible_size(&self) -> IntSize {
        IntSize::new(self.bounds.width().round() as i32, self.bounds.height().round() as i32)
    }

    fn advance(&mut self, duration_millis: i64) {
        assert!(duration_millis >= 0, "duration of a delay can only be positive, not {duration_millis}");
        self.current_time += duration_millis;
    }

    fn within_root(&self, position: Offset) -> bool {
        Rect::new(0.0, 0.0, self.root_size.width as f32, self.root_size.height as f32).contains(position)
    }
}

/// Sends the queued events at their times, running the rule's clock up to each; the state saved.
fn flush(rule: &KompozTestRule, events: Vec<InjectedEvent>, mut state: InjectionState) {
    let host = rule.host();
    for event in events {
        let time = match &event {
            InjectedEvent::TouchDown { time, .. }
            | InjectedEvent::TouchMove { time, .. }
            | InjectedEvent::TouchUp { time, .. }
            | InjectedEvent::TouchCancel { time }
            | InjectedEvent::MouseMove { time, .. }
            | InjectedEvent::MouseButton { time, .. }
            | InjectedEvent::MouseCancel { time }
            | InjectedEvent::MouseExit { time, .. }
            | InjectedEvent::MouseScroll { time, .. } => *time,
        };
        let nanos = time * 1_000_000;
        while rule.clock.get() + FRAME_NANOS <= nanos {
            rule.advance_frame();
        }
        match event {
            InjectedEvent::TouchDown { pointer_id, position, .. } => {
                host.pointer_down(position, nanos, pointer_id, PointerType::Touch);
            }
            InjectedEvent::TouchMove { positions, .. } => {
                host.pointer_moves(&positions, nanos);
            }
            InjectedEvent::TouchUp { pointer_id, position, .. } => {
                host.pointer_up(position, nanos, pointer_id);
            }
            InjectedEvent::TouchCancel { .. } | InjectedEvent::MouseCancel { .. } => {
                host.dispatcher().advance_to(nanos);
                host.pointer_cancel();
            }
            InjectedEvent::MouseMove { position, .. } => {
                host.pointer_move(position, nanos, MOUSE_POINTER_ID);
            }
            InjectedEvent::MouseButton { position, button, pressed, .. } => {
                host.mouse_button(position, nanos, button, pressed);
            }
            InjectedEvent::MouseExit { position, .. } => {
                host.pointer_move(position, nanos, MOUSE_POINTER_ID);
                host.pointer_exit();
            }
            InjectedEvent::MouseScroll { position, delta, .. } => {
                host.dispatcher().advance_to(nanos);
                host.scroll(position, delta);
            }
        }
        state.last_event_time = time;
    }
    // What the events woke runs now (Compose's `runCurrent`).
    host.dispatcher().run_ready();
    *rule.injection.borrow_mut() = state;
}

// ---------------------------------------------------------------------------------------------------------------
// Touch

/// Where `perform_touch_input` injects a finger gesture (Compose's TouchInjectionScope): pointers by id (0 unless
/// said), positions local to the node.
pub struct TouchInjectionScope {
    base: ScopeBase,
    touches: Vec<(i32, Offset)>,
}

impl InjectionScope for TouchInjectionScope {
    fn advance_event_time(&mut self, duration_millis: i64) {
        self.base.advance(duration_millis);
    }
    fn visible_size(&self) -> IntSize {
        self.base.visible_size()
    }
    fn view_configuration(&self) -> ViewConfiguration {
        self.base.view_configuration
    }
    fn density(&self) -> Density {
        self.base.density
    }
}

impl TouchInjectionScope {
    fn position_of(&self, pointer_id: i32) -> Option<Offset> {
        self.touches.iter().find(|(id, _)| *id == pointer_id).map(|(_, p)| *p)
    }

    /// Where the pointer is (local to the node), if it is down (Compose's `currentPosition`).
    pub fn current_position(&self, pointer_id: i32) -> Option<Offset> {
        self.position_of(pointer_id).map(|p| self.base.root_to_local(p))
    }

    /// Pointer 0 goes down at `position` (Compose's `down(position)`).
    pub fn down(&mut self, position: Offset) {
        self.down_pointer(0, position);
    }

    /// A pointer goes down (Compose's `down(pointerId, position)`): it mustn't be down already.
    pub fn down_pointer(&mut self, pointer_id: i32, position: Offset) {
        assert!(
            self.position_of(pointer_id).is_none(),
            "Cannot send DOWN event, a gesture is already in progress for pointer {pointer_id}"
        );
        let root = self.base.local_to_root(position);
        self.touches.push((pointer_id, root));
        self.base.events.push(InjectedEvent::TouchDown {
            time: self.base.current_time,
            pointer_id: pointer_id as i64,
            position: root,
        });
    }

    /// Changes where a pointer is without sending anything (Compose's `updatePointerTo`): the next `move` does.
    pub fn update_pointer_to(&mut self, pointer_id: i32, position: Offset) {
        let root = self.base.local_to_root(position);
        match self.touches.iter_mut().find(|(id, _)| *id == pointer_id) {
            Some(p) => p.1 = root,
            None => panic!("Cannot move pointer {pointer_id}, it is not active in the current gesture"),
        }
    }

    /// The same by `delta` (Compose's `updatePointerBy`).
    pub fn update_pointer_by(&mut self, pointer_id: i32, delta: Offset) {
        let current = self.current_position(pointer_id).unwrap_or(Offset::Zero);
        self.update_pointer_to(pointer_id, current + delta);
    }

    /// The pointers' moves, `delay_millis` after the last event: one event of all of them (Compose's `move`).
    pub fn r#move(&mut self, delay_millis: i64) {
        assert!(!self.touches.is_empty(), "Cannot send MOVE event, no gesture is in progress");
        self.base.advance(delay_millis);
        let positions = self.touches.iter().map(|(id, p)| (*id as i64, *p)).collect();
        self.base.events.push(InjectedEvent::TouchMove { time: self.base.current_time, positions });
    }

    /// Pointer 0 moves to `position` an event period on (Compose's `moveTo(position)`).
    pub fn move_to(&mut self, position: Offset) {
        self.move_to_pointer(0, position, EVENT_PERIOD_MILLIS);
    }

    /// A pointer moves to `position` `delay_millis` on (Compose's `moveTo(pointerId, position, delayMillis)`).
    pub fn move_to_pointer(&mut self, pointer_id: i32, position: Offset, delay_millis: i64) {
        self.update_pointer_to(pointer_id, position);
        self.r#move(delay_millis);
    }

    /// Pointer 0 moves by `delta` an event period on (Compose's `moveBy(delta)`).
    pub fn move_by(&mut self, delta: Offset) {
        self.move_by_pointer(0, delta, EVENT_PERIOD_MILLIS);
    }

    /// A pointer moves by `delta` `delay_millis` on.
    pub fn move_by_pointer(&mut self, pointer_id: i32, delta: Offset, delay_millis: i64) {
        self.update_pointer_by(pointer_id, delta);
        self.r#move(delay_millis);
    }

    /// A pointer goes up where it is, at once (Compose's `up(pointerId)`).
    pub fn up_pointer(&mut self, pointer_id: i32) {
        let Some(position) = self.position_of(pointer_id) else {
            panic!("Cannot send UP event for pointer {pointer_id}, it is not active in the current gesture")
        };
        self.touches.retain(|(id, _)| *id != pointer_id);
        self.base.events.push(InjectedEvent::TouchUp {
            time: self.base.current_time,
            pointer_id: pointer_id as i64,
            position,
        });
    }

    /// Pointer 0 goes up (Compose's `up()`).
    pub fn up(&mut self) {
        self.up_pointer(0);
    }

    /// The gesture is cancelled `delay_millis` on (Compose's `cancel`).
    pub fn cancel_after(&mut self, delay_millis: i64) {
        assert!(!self.touches.is_empty(), "Cannot send CANCEL event, no gesture is in progress");
        self.base.advance(delay_millis);
        self.touches.clear();
        self.base.events.push(InjectedEvent::TouchCancel { time: self.base.current_time });
    }

    /// The gesture is cancelled an event period on (Compose's `cancel()`).
    pub fn cancel(&mut self) {
        self.cancel_after(EVENT_PERIOD_MILLIS);
    }

    /// A down, a move an event period on (where it is), an up (Compose's `click(position)`).
    pub fn click_with(&mut self, position: Offset) {
        self.down(position);
        self.r#move(EVENT_PERIOD_MILLIS);
        self.up();
    }

    /// Held still for `duration_millis`, at least the long press timeout (Compose's `longClick`).
    pub fn long_click_with(&mut self, position: Offset, duration_millis: i64) {
        let timeout = self.base.view_configuration.long_press_timeout_millis();
        assert!(duration_millis >= timeout, "Long click must have a duration of at least {timeout}ms");
        self.swipe_with(position, position, duration_millis);
    }

    /// Two clicks `delay_millis` apart, between the double tap minimum and timeout (Compose's `doubleClick`).
    pub fn double_click_with(&mut self, position: Offset, delay_millis: i64) {
        let vc = self.base.view_configuration;
        assert!(
            delay_millis >= vc.double_tap_min_time_millis(),
            "Time between clicks in double click must be at least {}ms",
            vc.double_tap_min_time_millis()
        );
        assert!(
            delay_millis < vc.double_tap_timeout_millis(),
            "Time between clicks in double click must be smaller than {}ms",
            vc.double_tap_timeout_millis()
        );
        self.click_with(position);
        self.base.advance(delay_millis);
        self.click_with(position);
    }

    /// From `start` to `end` in `duration_millis`, a move each event period (Compose's `swipe`).
    pub fn swipe_with(&mut self, start: Offset, end: Offset, duration_millis: i64) {
        let d = duration_millis as f32;
        self.swipe_along(move |t| lerp_offset(start, end, t as f32 / d), duration_millis, &[]);
    }

    /// Along `curve` (position at each time since the start), a move each event period and at each of
    /// `key_times` (Compose's `swipe(curve, durationMillis, keyTimes)`).
    pub fn swipe_along(&mut self, curve: impl Fn(i64) -> Offset + 'static, duration_millis: i64, key_times: &[i64]) {
        self.multi_touch_swipe(vec![Box::new(curve)], duration_millis, key_times);
    }

    /// Pointers 0, 1, … along `curves` at once (Compose's `multiTouchSwipe`).
    pub fn multi_touch_swipe(
        &mut self,
        curves: Vec<Box<dyn Fn(i64) -> Offset>>,
        duration_millis: i64,
        key_times: &[i64],
    ) {
        assert!(duration_millis >= 1, "duration must be at least 1 millisecond, not {duration_millis}");
        assert!(
            key_times.iter().all(|t| (0..=duration_millis).contains(t)),
            "keyTimes contains timestamps out of range [0..{duration_millis}]: {key_times:?}"
        );
        assert!(key_times.windows(2).all(|w| w[0] <= w[1]), "keyTimes must be sorted: {key_times:?}");
        for (i, curve) in curves.iter().enumerate() {
            self.down_pointer(i as i32, curve(0));
        }
        let mut current = 0;
        let mut key = 0;
        while current < duration_millis {
            while key < key_times.len() && key_times[key] <= current {
                key += 1;
            }
            let next = if key < key_times.len() { key_times[key] } else { duration_millis };
            self.send_swipe_segment(&curves, current, next);
            current = next;
        }
        for i in 0..curves.len() {
            self.up_pointer(i as i32);
        }
    }

    /// Compose's `sendMultiTouchSwipeSegment`.
    fn send_swipe_segment(&mut self, curves: &[Box<dyn Fn(i64) -> Offset>], t0: i64, tn: i64) {
        let steps = (((tn - t0) as f32 / EVENT_PERIOD_MILLIS as f32).round() as i64).max(1);
        let mut previous = t0;
        for step in 1..=steps {
            let t = lerp_time(t0, tn, step as f32 / steps as f32);
            for (i, curve) in curves.iter().enumerate() {
                self.update_pointer_to(i as i32, curve(t));
            }
            self.r#move(t - previous);
            previous = t;
        }
    }

    /// Two pointers from `start0` and `start1` to `end0` and `end1` (Compose's `pinch`).
    pub fn pinch_with(&mut self, start0: Offset, end0: Offset, start1: Offset, end1: Offset, duration_millis: i64) {
        let d = duration_millis as f32;
        self.multi_touch_swipe(
            vec![
                Box::new(move |t| lerp_offset(start0, end0, t as f32 / d)),
                Box::new(move |t| lerp_offset(start1, end1, t as f32 / d)),
            ],
            duration_millis,
            &[],
        );
    }

    /// Up the middle from `start_y` to `end_y` (Compose's `swipeUp`).
    pub fn swipe_up_with(&mut self, start_y: f32, end_y: f32, duration_millis: i64) {
        assert!(start_y >= end_y, "startY={start_y} needs to be greater than or equal to endY={end_y}");
        let x = self.center_x();
        self.swipe_with(Offset::new(x, start_y), Offset::new(x, end_y), duration_millis);
    }

    /// Down the middle (Compose's `swipeDown`).
    pub fn swipe_down_with(&mut self, start_y: f32, end_y: f32, duration_millis: i64) {
        assert!(start_y <= end_y, "startY={start_y} needs to be less than or equal to endY={end_y}");
        let x = self.center_x();
        self.swipe_with(Offset::new(x, start_y), Offset::new(x, end_y), duration_millis);
    }

    /// Leftwards across the middle (Compose's `swipeLeft`).
    pub fn swipe_left_with(&mut self, start_x: f32, end_x: f32, duration_millis: i64) {
        assert!(start_x >= end_x, "startX={start_x} needs to be greater than or equal to endX={end_x}");
        let y = self.center_y();
        self.swipe_with(Offset::new(start_x, y), Offset::new(end_x, y), duration_millis);
    }

    /// Rightwards across the middle (Compose's `swipeRight`).
    pub fn swipe_right_with(&mut self, start_x: f32, end_x: f32, duration_millis: i64) {
        assert!(start_x <= end_x, "startX={start_x} needs to be less than or equal to endX={end_x}");
        let y = self.center_y();
        self.swipe_with(Offset::new(start_x, y), Offset::new(end_x, y), duration_millis);
    }
}

/// The touch gestures with Compose's defaults, named in a kompozable body (`click(position = …)`); plain code calls
/// them for the defaults and the `…_with` ones for the rest.
#[named]
impl TouchInjectionScope {
    /// A click at the center by default (Compose's `click`).
    pub fn click(&mut self, #[default(self.center())] position: Offset) {
        self.click_with(position);
    }

    /// A long click: the long press timeout and 100 ms more by default (Compose's `longClick`).
    pub fn long_click(
        &mut self,
        #[default(self.center())] position: Offset,
        #[default(self.view_configuration().long_press_timeout_millis() + 100)] duration_millis: i64,
    ) {
        self.long_click_with(position, duration_millis);
    }

    /// A double click, half way between the double tap minimum and timeout by default (Compose's `doubleClick`).
    pub fn double_click(
        &mut self,
        #[default(self.center())] position: Offset,
        #[default((self.view_configuration().double_tap_min_time_millis()
            + self.view_configuration().double_tap_timeout_millis()) / 2)]
        delay_millis: i64,
    ) {
        self.double_click_with(position, delay_millis);
    }

    /// A swipe, 200 ms by default.
    pub fn swipe(&mut self, start: Offset, end: Offset, #[default(200)] duration_millis: i64) {
        self.swipe_with(start, end, duration_millis);
    }

    /// A pinch, 400 ms by default.
    pub fn pinch(
        &mut self,
        start0: Offset,
        end0: Offset,
        start1: Offset,
        end1: Offset,
        #[default(400)] duration_millis: i64,
    ) {
        self.pinch_with(start0, end0, start1, end1, duration_millis);
    }

    /// Up the middle, bottom to top in 200 ms by default.
    pub fn swipe_up(
        &mut self,
        #[default(self.bottom())] start_y: f32,
        #[default(self.top())] end_y: f32,
        #[default(200)] duration_millis: i64,
    ) {
        self.swipe_up_with(start_y, end_y, duration_millis);
    }

    /// Down the middle, top to bottom by default.
    pub fn swipe_down(
        &mut self,
        #[default(self.top())] start_y: f32,
        #[default(self.bottom())] end_y: f32,
        #[default(200)] duration_millis: i64,
    ) {
        self.swipe_down_with(start_y, end_y, duration_millis);
    }

    /// Leftwards, right to left by default.
    pub fn swipe_left(
        &mut self,
        #[default(self.right())] start_x: f32,
        #[default(self.left())] end_x: f32,
        #[default(200)] duration_millis: i64,
    ) {
        self.swipe_left_with(start_x, end_x, duration_millis);
    }

    /// Rightwards, left to right by default.
    pub fn swipe_right(
        &mut self,
        #[default(self.left())] start_x: f32,
        #[default(self.right())] end_x: f32,
        #[default(200)] duration_millis: i64,
    ) {
        self.swipe_right_with(start_x, end_x, duration_millis);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Mouse

/// A mouse's wheel (Compose's ScrollWheel).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollWheel {
    Vertical,
    Horizontal,
}

/// The time between a click's press and release, ms (Compose's `SingleClickDelayMillis`).
const SINGLE_CLICK_DELAY_MILLIS: i64 = 60;
/// A mouse gesture's default length, ms (Compose's `DefaultMouseGestureDurationMillis`).
const DEFAULT_MOUSE_GESTURE_DURATION_MILLIS: i64 = 300;

/// Where `perform_mouse_input` injects mouse input (Compose's MouseInjectionScope): positions local to the node.
pub struct MouseInjectionScope {
    base: ScopeBase,
    cursor: Offset,
    cursor_in: bool,
    buttons: PointerButtons,
}

impl InjectionScope for MouseInjectionScope {
    fn advance_event_time(&mut self, duration_millis: i64) {
        self.base.advance(duration_millis);
    }
    fn visible_size(&self) -> IntSize {
        self.base.visible_size()
    }
    fn view_configuration(&self) -> ViewConfiguration {
        self.base.view_configuration
    }
    fn density(&self) -> Density {
        self.base.density
    }
}

impl MouseInjectionScope {
    /// Where the cursor is, local to the node (Compose's `currentPosition`).
    pub fn current_position(&self) -> Offset {
        self.base.root_to_local(self.cursor)
    }

    /// The cursor moves to `position` `delay_millis` on (Compose's `moveTo`).
    pub fn move_to_with(&mut self, position: Offset, delay_millis: i64) {
        self.base.advance(delay_millis);
        self.cursor = self.base.local_to_root(position);
        self.cursor_in = true;
        self.base.events.push(InjectedEvent::MouseMove { time: self.base.current_time, position: self.cursor });
    }

    /// The cursor moves an event period on.
    pub fn move_to(&mut self, position: Offset) {
        self.move_to_with(position, EVENT_PERIOD_MILLIS);
    }

    /// The cursor moves by `delta` (Compose's `moveBy`).
    pub fn move_by_with(&mut self, delta: Offset, delay_millis: i64) {
        let to = self.current_position() + delta;
        self.move_to_with(to, delay_millis);
    }

    pub fn move_by(&mut self, delta: Offset) {
        self.move_by_with(delta, EVENT_PERIOD_MILLIS);
    }

    /// Puts the cursor at `position` without sending anything (Compose's `updatePointerTo`).
    pub fn update_pointer_to(&mut self, position: Offset) {
        self.cursor = self.base.local_to_root(position);
    }

    pub fn update_pointer_by(&mut self, delta: Offset) {
        let to = self.current_position() + delta;
        self.update_pointer_to(to);
    }

    /// Presses `button` where the cursor is (Compose's `press`).
    pub fn press_button(&mut self, button: PointerButton) {
        assert!(
            self.base.within_root(self.cursor) || self.buttons.are_any_pressed(),
            "Cannot start a mouse gesture outside the Compose root bounds, mouse position is {:?} and bounds are {:?}",
            self.cursor,
            self.base.root_size
        );
        assert!(
            !self.buttons.is_pressed(button.0),
            "Cannot send mouse button down event, button {} is already pressed",
            button.0
        );
        self.buttons = PointerButtons(self.buttons.0 | 1 << button.0);
        self.cursor_in = true;
        self.base.events.push(InjectedEvent::MouseButton {
            time: self.base.current_time,
            position: self.cursor,
            button,
            pressed: true,
        });
    }

    /// Lets go of `button` (Compose's `release`).
    pub fn release_button(&mut self, button: PointerButton) {
        assert!(
            self.buttons.is_pressed(button.0),
            "Cannot send mouse button up event, button {} is not pressed",
            button.0
        );
        self.buttons = PointerButtons(self.buttons.0 & !(1 << button.0));
        self.base.events.push(InjectedEvent::MouseButton {
            time: self.base.current_time,
            position: self.cursor,
            button,
            pressed: false,
        });
    }

    /// The mouse gesture is cancelled `delay_millis` on (Compose's `cancel`).
    pub fn cancel_after(&mut self, delay_millis: i64) {
        assert!(self.buttons.are_any_pressed(), "Cannot send mouse cancel event, no mouse buttons are pressed");
        self.base.advance(delay_millis);
        self.buttons = PointerButtons::None;
        self.base.events.push(InjectedEvent::MouseCancel { time: self.base.current_time });
    }

    pub fn cancel(&mut self) {
        self.cancel_after(EVENT_PERIOD_MILLIS);
    }

    /// The cursor comes over the window at `position` `delay_millis` on (Compose's `enter`).
    pub fn enter_with(&mut self, position: Offset, delay_millis: i64) {
        assert!(!self.cursor_in, "Cannot send mouse hover enter event, mouse is already hovering");
        self.move_to_with(position, delay_millis);
    }

    /// The cursor leaves the window from `position` `delay_millis` on (Compose's `exit`).
    pub fn exit_with(&mut self, position: Offset, delay_millis: i64) {
        assert!(self.cursor_in, "Cannot send mouse hover exit event, mouse is not hovering");
        self.base.advance(delay_millis);
        self.cursor = self.base.local_to_root(position);
        self.cursor_in = false;
        self.base.events.push(InjectedEvent::MouseExit { time: self.base.current_time, position: self.cursor });
    }

    /// The wheel turned by `delta` px (down or right positive: Compose's `scroll(delta, scrollWheel)`).
    pub fn scroll_with(&mut self, delta: f32, scroll_wheel: ScrollWheel) {
        let d = match scroll_wheel {
            ScrollWheel::Vertical => Offset::new(0.0, delta),
            ScrollWheel::Horizontal => Offset::new(delta, 0.0),
        };
        self.scroll_offset(d);
    }

    /// Both wheels at once (Compose's `scroll(offset)`).
    pub fn scroll_offset(&mut self, delta: Offset) {
        if self.base.within_root(self.cursor) {
            self.cursor_in = true;
            self.base.events.push(InjectedEvent::MouseScroll {
                time: self.base.current_time,
                position: self.cursor,
                delta,
            });
        }
    }

    /// A press and, 60 ms on, a release of `button` at `position` (Compose's `click`).
    pub fn click_with(&mut self, position: Offset, button: PointerButton) {
        if position.is_specified() {
            self.update_pointer_to(position);
        }
        self.press_button(button);
        self.base.advance(SINGLE_CLICK_DELAY_MILLIS);
        self.release_button(button);
    }

    /// Two clicks the default double tap delay apart (Compose's `doubleClick`).
    pub fn double_click_with(&mut self, position: Offset, button: PointerButton) {
        let delay = self.default_double_tap_delay();
        self.click_with(position, button);
        self.base.advance(delay);
        self.click_with(position, button);
    }

    /// Three clicks (Compose's `tripleClick`).
    pub fn triple_click_with(&mut self, position: Offset, button: PointerButton) {
        let delay = self.default_double_tap_delay();
        self.click_with(position, button);
        self.base.advance(delay);
        self.click_with(position, button);
        self.base.advance(delay);
        self.click_with(position, button);
    }

    /// Pressed for the long press timeout and 100 ms more (Compose's `longClick`).
    pub fn long_click_with(&mut self, position: Offset, button: PointerButton) {
        if position.is_specified() {
            self.update_pointer_to(position);
        }
        self.press_button(button);
        let wait = self.base.view_configuration.long_press_timeout_millis() + 100;
        self.base.advance(wait);
        self.release_button(button);
    }

    fn default_double_tap_delay(&self) -> i64 {
        let vc = self.base.view_configuration;
        (vc.double_tap_min_time_millis() + vc.double_tap_timeout_millis()) / 2
    }

    /// The cursor along `curve` in `duration_millis`, a move each event period (Compose's `animateMoveAlong`).
    pub fn animate_move_along(&mut self, curve: impl Fn(i64) -> Offset, duration_millis: i64) {
        assert!(duration_millis > 0, "Duration is 0");
        let start = curve(0);
        if start != self.current_position() {
            self.move_to_with(start, 0);
        }
        let steps = ((duration_millis as f32 / EVENT_PERIOD_MILLIS as f32).round() as i64).max(1);
        let mut previous = 0;
        for step in 1..=steps {
            let t = lerp_time(0, duration_millis, step as f32 / steps as f32);
            self.move_to_with(curve(t), t - previous);
            previous = t;
        }
    }

    /// The cursor to `position` in `duration_millis` (Compose's `animateMoveTo`).
    pub fn animate_move_to_with(&mut self, position: Offset, duration_millis: i64) {
        let start = self.current_position();
        let d = duration_millis as f32;
        self.animate_move_along(move |t| lerp_offset(start, position, t as f32 / d), duration_millis);
    }

    /// Presses `button` at `start`, moves to `end`, lets go (Compose's `dragAndDrop`).
    pub fn drag_and_drop_with(&mut self, start: Offset, end: Offset, button: PointerButton, duration_millis: i64) {
        self.update_pointer_to(start);
        self.press_button(button);
        self.animate_move_to_with(end, duration_millis);
        self.release_button(button);
    }

    /// The wheel turned `scroll_amount` in steps over `duration_millis` (Compose's `smoothScroll`).
    pub fn smooth_scroll_with(&mut self, scroll_amount: f32, duration_millis: i64, scroll_wheel: ScrollWheel) {
        let steps = ((duration_millis as f32 / EVENT_PERIOD_MILLIS as f32).round() as i64).max(1);
        let (mut previous_t, mut previous_value) = (0, 0.0f32);
        for step in 1..=steps {
            let progress = step as f32 / steps as f32;
            let t = lerp_time(0, duration_millis, progress);
            let value = scroll_amount * progress;
            self.base.advance(t - previous_t);
            self.scroll_with(value - previous_value, scroll_wheel);
            previous_t = t;
            previous_value = value;
        }
    }
}

/// The mouse input with Compose's defaults, named in a kompozable body; plain code calls them for the defaults and
/// the `…_with` ones for the rest.
#[named]
impl MouseInjectionScope {
    /// Presses a button, the primary one by default (Compose's `press`).
    pub fn press(&mut self, #[default(PointerButton::Primary)] button: PointerButton) {
        self.press_button(button);
    }

    /// Lets go of a button, the primary one by default.
    pub fn release(&mut self, #[default(PointerButton::Primary)] button: PointerButton) {
        self.release_button(button);
    }

    /// The cursor comes over the window, where it is by default.
    pub fn enter(&mut self, #[default(self.current_position())] position: Offset) {
        self.enter_with(position, EVENT_PERIOD_MILLIS);
    }

    /// The cursor leaves the window, from where it is by default.
    pub fn exit(&mut self, #[default(self.current_position())] position: Offset) {
        self.exit_with(position, EVENT_PERIOD_MILLIS);
    }

    /// The vertical wheel by default.
    pub fn scroll(&mut self, delta: f32, #[default(ScrollWheel::Vertical)] scroll_wheel: ScrollWheel) {
        self.scroll_with(delta, scroll_wheel);
    }

    /// A click at the center with the primary button by default.
    pub fn click(
        &mut self,
        #[default(self.center())] position: Offset,
        #[default(PointerButton::Primary)] button: PointerButton,
    ) {
        self.click_with(position, button);
    }

    /// A click of the secondary button (Compose's `rightClick`).
    pub fn right_click(&mut self, #[default(self.center())] position: Offset) {
        self.click_with(position, PointerButton::Secondary);
    }

    pub fn double_click(
        &mut self,
        #[default(self.center())] position: Offset,
        #[default(PointerButton::Primary)] button: PointerButton,
    ) {
        self.double_click_with(position, button);
    }

    pub fn triple_click(
        &mut self,
        #[default(self.center())] position: Offset,
        #[default(PointerButton::Primary)] button: PointerButton,
    ) {
        self.triple_click_with(position, button);
    }

    pub fn long_click(
        &mut self,
        #[default(self.center())] position: Offset,
        #[default(PointerButton::Primary)] button: PointerButton,
    ) {
        self.long_click_with(position, button);
    }

    /// The cursor to `position`, 300 ms by default.
    pub fn animate_move_to(
        &mut self,
        position: Offset,
        #[default(DEFAULT_MOUSE_GESTURE_DURATION_MILLIS)] duration_millis: i64,
    ) {
        self.animate_move_to_with(position, duration_millis);
    }

    /// The cursor by `delta`, 300 ms by default (Compose's `animateMoveBy`).
    pub fn animate_move_by(
        &mut self,
        delta: Offset,
        #[default(DEFAULT_MOUSE_GESTURE_DURATION_MILLIS)] duration_millis: i64,
    ) {
        let to = self.current_position() + delta;
        self.animate_move_to_with(to, duration_millis);
    }

    pub fn drag_and_drop(
        &mut self,
        start: Offset,
        end: Offset,
        #[default(PointerButton::Primary)] button: PointerButton,
        #[default(DEFAULT_MOUSE_GESTURE_DURATION_MILLIS)] duration_millis: i64,
    ) {
        self.drag_and_drop_with(start, end, button, duration_millis);
    }

    pub fn smooth_scroll(
        &mut self,
        scroll_amount: f32,
        #[default(DEFAULT_MOUSE_GESTURE_DURATION_MILLIS)] duration_millis: i64,
        #[default(ScrollWheel::Vertical)] scroll_wheel: ScrollWheel,
    ) {
        self.smooth_scroll_with(scroll_amount, duration_millis, scroll_wheel);
    }
}

impl SemanticsNodeInteraction<'_> {
    /// Injects a finger gesture on the node (Compose's `performTouchInput`): `block` queues it, then it is sent.
    #[track_caller]
    pub fn perform_touch_input(&self, block: impl FnOnce(&mut TouchInjectionScope)) -> &Self {
        let (base, mut state) = ScopeBase::new(self);
        let mut scope = TouchInjectionScope { base, touches: std::mem::take(&mut state.touches) };
        block(&mut scope);
        state.touches = scope.touches;
        flush(self.rule, scope.base.events, state);
        self
    }

    /// Injects mouse input on the node (Compose's `performMouseInput`).
    #[track_caller]
    pub fn perform_mouse_input(&self, block: impl FnOnce(&mut MouseInjectionScope)) -> &Self {
        let (base, state) = ScopeBase::new(self);
        let mut scope =
            MouseInjectionScope { base, cursor: state.cursor, cursor_in: state.cursor_in, buttons: state.buttons };
        block(&mut scope);
        let state = InjectionState {
            touches: state.touches,
            cursor: scope.cursor,
            cursor_in: scope.cursor_in,
            buttons: scope.buttons,
            last_event_time: state.last_event_time,
        };
        flush(self.rule, scope.base.events, state);
        self
    }
}
