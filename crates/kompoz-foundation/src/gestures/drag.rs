//! Drags, as Compose's `DragGestureDetector.kt`: the touch slop (`await_touch_slop_or_cancellation`), following a
//! pointer (`await_drag_or_cancellation`, `drag`), and the detectors made of them.

use super::Orientation;
use super::for_each::await_each_gesture;
use super::tap::await_first_down;
use kompoz_graphics::Offset;
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerEvent, PointerEventPass, PointerId, PointerInputChange, PointerInputScope,
    PointerType,
};
use kompoz_ui::platform::ViewConfiguration;
use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::rc::Rc;

/// The default touch slop on Android devices, and a mouse's (Compose's `defaultTouchSlop`, `mouseSlop`, dp).
const DEFAULT_TOUCH_SLOP: f32 = 18.0;
const MOUSE_SLOP: f32 = 0.125;

/// How far a pointer of a type moves before it drags (Compose's `ViewConfiguration.pointerSlop`): a mouse's is a
/// fraction of a finger's.
pub(crate) fn pointer_slop(vc: &ViewConfiguration, pointer_type: PointerType) -> f32 {
    match pointer_type {
        PointerType::Mouse => vc.touch_slop * (MOUSE_SLOP / DEFAULT_TOUCH_SLOP),
        _ => vc.touch_slop,
    }
}

/// Whether the pointer isn't pressed in `event` (Compose's `isPointerUp`).
fn is_pointer_up(event: &PointerEvent, pointer_id: PointerId) -> bool {
    event.changes.iter().find(|c| c.id == pointer_id).is_none_or(|c| !c.pressed)
}

/// Adds up a pointer's moves until they pass the touch slop (Compose's TouchSlopDetector), along an axis or any
/// way.
pub(crate) struct TouchSlopDetector {
    orientation: Option<Orientation>,
    total_position_change: Offset,
}

impl TouchSlopDetector {
    pub(crate) fn new(orientation: Option<Orientation>, initial_position_change: Offset) -> TouchSlopDetector {
        TouchSlopDetector { orientation, total_position_change: initial_position_change }
    }

    fn main_axis(&self, o: Offset) -> f32 {
        if self.orientation == Some(Orientation::Horizontal) { o.x } else { o.y }
    }

    fn cross_axis(&self, o: Offset) -> f32 {
        if self.orientation == Some(Orientation::Horizontal) { o.y } else { o.x }
    }

    /// How far past the slop the moves went with `position_change`, `Offset::Unspecified` while short of it
    /// (Compose's `getPostSlopOffset`).
    pub(crate) fn get_post_slop_offset(&mut self, position_change: Offset, touch_slop: f32) -> Offset {
        self.total_position_change = self.total_position_change + position_change;
        let final_change = self.total_position_change;
        let in_direction = match self.orientation {
            None => final_change.get_distance(),
            Some(_) => self.main_axis(final_change).abs(),
        };
        if in_direction > 0.0 && in_direction >= touch_slop {
            self.calculate_post_slop_offset(touch_slop)
        } else {
            Offset::Unspecified
        }
    }

    pub(crate) fn reset(&mut self) {
        self.total_position_change = Offset::Zero;
    }

    fn calculate_post_slop_offset(&self, touch_slop: f32) -> Offset {
        let total = self.total_position_change;
        match self.orientation {
            None => {
                let slop = total / total.get_distance() * touch_slop;
                total - slop
            }
            Some(o) => {
                let main = self.main_axis(total);
                let final_main = main - main.signum() * touch_slop;
                let cross = self.cross_axis(total);
                if o == Orientation::Horizontal {
                    Offset::new(final_main, cross)
                } else {
                    Offset::new(cross, final_main)
                }
            }
        }
    }
}

/// Compose's `awaitPointerSlopOrCancellation`.
pub(crate) async fn await_pointer_slop_or_cancellation(
    s: &AwaitPointerEventScope,
    pointer_id: PointerId,
    pointer_type: PointerType,
    orientation: Option<Orientation>,
    initial_position_change: Offset,
    on_pointer_slop_reached: impl Fn(&PointerInputChange, Offset),
) -> Option<PointerInputChange> {
    if is_pointer_up(&s.current_event(), pointer_id) {
        return None; // the pointer was lifted already: cancelled
    }
    let touch_slop = pointer_slop(&s.view_configuration(), pointer_type);
    let mut pointer = pointer_id;
    let mut detector = TouchSlopDetector::new(orientation, initial_position_change);
    loop {
        let event = s.await_pointer_event().await;
        let drag_event = event.changes.iter().find(|c| c.id == pointer)?.clone();
        if drag_event.is_consumed() {
            return None;
        } else if drag_event.changed_to_up_ignore_consumed() {
            let other_down = event.changes.iter().find(|c| c.pressed)?;
            pointer = other_down.id;
        } else {
            let post_slop = detector.get_post_slop_offset(drag_event.position_change_ignore_consumed(), touch_slop);
            if post_slop.is_specified() {
                on_pointer_slop_reached(&drag_event, post_slop);
                if drag_event.is_consumed() {
                    return Some(drag_event);
                }
                detector.reset();
            } else {
                // Taken in the Final pass: cancelled.
                s.await_pointer_event_with(PointerEventPass::Final).await;
                if drag_event.is_consumed() {
                    return None;
                }
            }
        }
    }
}

/// The next change of the pointer that `has_dragged`, or its release once all are up; none if the pointer isn't
/// in the event (Compose's `awaitDragOrUp`).
async fn await_drag_or_up(
    s: &AwaitPointerEventScope,
    pointer_id: PointerId,
    has_dragged: impl Fn(&PointerInputChange) -> bool,
) -> Option<PointerInputChange> {
    let mut pointer = pointer_id;
    loop {
        let event = s.await_pointer_event().await;
        let drag_event = event.changes.iter().find(|c| c.id == pointer)?;
        if drag_event.changed_to_up_ignore_consumed() {
            match event.changes.iter().find(|c| c.pressed) {
                None => return Some(drag_event.clone()),
                Some(other) => pointer = other.id,
            }
        } else if has_dragged(drag_event) {
            return Some(drag_event.clone());
        }
    }
}

/// Compose's `awaitDragOrCancellation` (and its vertical and horizontal ones).
pub(crate) async fn await_drag_or_cancellation(
    s: &AwaitPointerEventScope,
    pointer_id: PointerId,
    orientation: Option<Orientation>,
) -> Option<PointerInputChange> {
    if is_pointer_up(&s.current_event(), pointer_id) {
        return None;
    }
    let change = await_drag_or_up(s, pointer_id, |c| {
        let d = c.position_change_ignore_consumed();
        match orientation {
            None => c.position_changed_ignore_consumed(),
            Some(Orientation::Vertical) => d.y != 0.0,
            Some(Orientation::Horizontal) => d.x != 0.0,
        }
    })
    .await;
    change.filter(|c| !c.is_consumed())
}

/// Follows a drag (Compose's internal `drag`): its release, or none if a move was consumed or it was lifted
/// already.
pub(crate) async fn drag_internal(
    s: &AwaitPointerEventScope,
    pointer_id: PointerId,
    on_drag: impl Fn(&PointerInputChange),
    orientation: Option<Orientation>,
    motion_consumed: impl Fn(&PointerInputChange) -> bool,
) -> Option<PointerInputChange> {
    if is_pointer_up(&s.current_event(), pointer_id) {
        return None;
    }
    let mut pointer = pointer_id;
    loop {
        let change = await_drag_or_up(s, pointer, |c| {
            let d = c.position_change_ignore_consumed();
            let motion = match orientation {
                None => d.get_distance(),
                Some(Orientation::Vertical) => d.y,
                Some(Orientation::Horizontal) => d.x,
            };
            motion != 0.0
        })
        .await?;
        if motion_consumed(&change) {
            return None;
        }
        if change.changed_to_up_ignore_consumed() {
            return Some(change);
        }
        on_drag(&change);
        pointer = change.id;
    }
}

/// Compose's public `drag`: true when all pointers are up, false if it was cancelled.
pub(crate) async fn drag_until_up(
    s: &AwaitPointerEventScope,
    pointer_id: PointerId,
    on_drag: impl Fn(&PointerInputChange),
) -> bool {
    let mut pointer = pointer_id;
    loop {
        let Some(change) = await_drag_or_cancellation(s, pointer, None).await else { return false };
        if change.changed_to_up_ignore_consumed() {
            return true;
        }
        on_drag(&change);
        pointer = change.id;
    }
}

/// Compose's `verticalDrag` and `horizontalDrag`.
pub(crate) async fn axis_drag(
    s: &AwaitPointerEventScope,
    pointer_id: PointerId,
    orientation: Orientation,
    on_drag: impl Fn(&PointerInputChange),
) -> bool {
    drag_internal(s, pointer_id, on_drag, Some(orientation), |c| c.is_consumed()).await.is_some()
}

/// Compose's `awaitLongPressOrCancellation`.
pub(crate) async fn await_long_press_or_cancellation(
    s: &AwaitPointerEventScope,
    pointer_id: PointerId,
) -> Option<PointerInputChange> {
    let current = s.current_event();
    if is_pointer_up(&current, pointer_id) {
        return None; // lifted already: no long press
    }
    let initial_down = current.changes.iter().find(|c| c.id == pointer_id)?.clone();
    let long_press: Rc<std::cell::RefCell<Option<PointerInputChange>>> = Rc::new(std::cell::RefCell::new(None));
    let timeout = s.view_configuration().long_press_timeout_millis();
    let found = long_press.clone();
    let first = initial_down.clone();
    let waited = s
        .with_timeout(timeout, |s| async move {
            let mut current_down = first;
            let mut deep_press = false;
            let mut finished = false;
            while !finished {
                let event = s.await_pointer_event_with(PointerEventPass::Main).await;
                if event.changes.iter().all(|c| c.changed_to_up_ignore_consumed()) {
                    finished = true;
                }
                let (size, padding) = (s.size(), s.extended_touch_padding());
                if event.changes.iter().any(|c| c.is_consumed() || c.is_out_of_bounds(size, padding)) {
                    finished = true; // cancelled
                }
                if event.is_deep_press() {
                    deep_press = true;
                    finished = true;
                }
                let check = s.await_pointer_event_with(PointerEventPass::Final).await;
                if check.changes.iter().any(|c| c.is_consumed()) {
                    finished = true;
                }
                if is_pointer_up(&event, current_down.id) {
                    match event.changes.iter().find(|c| c.pressed) {
                        Some(new_pressed) => {
                            current_down = new_pressed.clone();
                            *found.borrow_mut() = Some(current_down.clone());
                        }
                        None => finished = true,
                    }
                } else {
                    *found.borrow_mut() = event.changes.iter().find(|c| c.id == current_down.id).cloned();
                }
            }
            deep_press
        })
        .await;
    let long_press = long_press.borrow_mut().take();
    match waited {
        Ok(true) => Some(long_press.unwrap_or(initial_down)),
        Ok(false) => None,
        Err(_) => Some(long_press.unwrap_or(initial_down)),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The detectors

type DragCallback = Rc<dyn Fn(&PointerInputChange, Offset)>;
type DragStartCallback = Rc<dyn Fn(&PointerInputChange, &PointerInputChange, Offset)>;

/// `detect_drag_gestures`'s callbacks (Compose's two `detectDragGestures` overloads: the plain one's
/// `on_drag_start(Offset)` and `on_drag_end()`, the other's `on_drag_start_with(down, slop_trigger, over_slop)`,
/// `on_drag_end_with(change)`, `orientation_lock` and `should_await_touch_slop`), then awaited.
pub struct DetectDragGestures {
    scope: PointerInputScope,
    orientation_lock: Option<Orientation>,
    on_drag_start: DragStartCallback,
    on_drag_end: Rc<dyn Fn(&PointerInputChange)>,
    on_drag_cancel: Rc<dyn Fn()>,
    should_await_touch_slop: Rc<dyn Fn() -> bool>,
    on_drag: DragCallback,
}

impl DetectDragGestures {
    pub(crate) fn new(scope: PointerInputScope, on_drag: impl Fn(&PointerInputChange, Offset) + 'static) -> Self {
        DetectDragGestures {
            scope,
            orientation_lock: None,
            on_drag_start: Rc::new(|_, _, _| {}),
            on_drag_end: Rc::new(|_| {}),
            on_drag_cancel: Rc::new(|| {}),
            should_await_touch_slop: Rc::new(|| true),
            on_drag: Rc::new(on_drag),
        }
    }

    /// The drag started, where the pointer passed the slop.
    pub fn on_drag_start(mut self, on_drag_start: impl Fn(Offset) + 'static) -> Self {
        self.on_drag_start = Rc::new(move |_, slop_trigger, _| on_drag_start(slop_trigger.position));
        self
    }

    /// The drag started: the first down, the change that passed the slop, and how far past it.
    pub fn on_drag_start_with(
        mut self,
        on_drag_start: impl Fn(&PointerInputChange, &PointerInputChange, Offset) + 'static,
    ) -> Self {
        self.on_drag_start = Rc::new(on_drag_start);
        self
    }

    /// The drag ended: all pointers up.
    pub fn on_drag_end(mut self, on_drag_end: impl Fn() + 'static) -> Self {
        self.on_drag_end = Rc::new(move |_| on_drag_end());
        self
    }

    /// The drag ended, with the last pointer's release.
    pub fn on_drag_end_with(mut self, on_drag_end: impl Fn(&PointerInputChange) + 'static) -> Self {
        self.on_drag_end = Rc::new(on_drag_end);
        self
    }

    /// The drag was taken by another (a move consumed).
    pub fn on_drag_cancel(mut self, on_drag_cancel: impl Fn() + 'static) -> Self {
        self.on_drag_cancel = Rc::new(on_drag_cancel);
        self
    }

    /// The slop counted along one axis only.
    pub fn orientation_lock(mut self, orientation_lock: Option<Orientation>) -> Self {
        self.orientation_lock = orientation_lock;
        self
    }

    /// Whether a gesture waits for the slop (asked at each press): without it, the press is the drag's start.
    pub fn should_await_touch_slop(mut self, should_await_touch_slop: impl Fn() -> bool + 'static) -> Self {
        self.should_await_touch_slop = Rc::new(should_await_touch_slop);
        self
    }
}

impl IntoFuture for DetectDragGestures {
    type Output = ();
    type IntoFuture = Pin<Box<dyn Future<Output = ()>>>;

    fn into_future(self) -> Self::IntoFuture {
        let d = Rc::new(self);
        Box::pin(async move {
            let scope = d.scope.clone();
            await_each_gesture(&scope, move |s| {
                let d = d.clone();
                async move {
                    let initial_down = await_first_down(&s, false, PointerEventPass::Initial).await;
                    process_drag_gesture(&s, &d, initial_down).await;
                }
            })
            .await
        })
    }
}

/// Compose's `processDragGesture`.
async fn process_drag_gesture(s: &AwaitPointerEventScope, d: &DetectDragGestures, initial_down: PointerInputChange) {
    let await_touch_slop = (d.should_await_touch_slop)();
    if !await_touch_slop {
        initial_down.consume();
    }
    let down = await_first_down(s, false, PointerEventPass::Main).await;
    let over_slop = std::cell::Cell::new(Offset::Zero);
    let mut drag: Option<PointerInputChange>;
    if await_touch_slop {
        loop {
            drag = await_pointer_slop_or_cancellation(
                s,
                down.id,
                down.r#type,
                d.orientation_lock,
                Offset::Zero,
                |c, over| {
                    c.consume();
                    over_slop.set(over);
                },
            )
            .await;
            if drag.as_ref().is_none_or(|c| c.is_consumed()) {
                break;
            }
        }
    } else {
        drag = Some(initial_down);
    }
    // Taken by another (a scroller) before the slop: when it lets go and pointers are still down, look again.
    while drag.is_none() && s.current_event().changes.iter().any(|c| c.pressed) {
        let mut event;
        loop {
            event = s.await_pointer_event_with(PointerEventPass::Final).await;
            let consumed = event.changes.iter().any(|c| c.is_consumed());
            let pressed = event.changes.iter().any(|c| c.pressed);
            if !(consumed && pressed) {
                break;
            }
        }
        if event.changes.iter().any(|c| c.pressed) {
            let initial_position_change = event.changes.first().map_or(Offset::Zero, |c| c.position) - down.position;
            drag = await_pointer_slop_or_cancellation(
                s,
                down.id,
                down.r#type,
                d.orientation_lock,
                initial_position_change,
                |c, _| {
                    c.consume();
                    over_slop.set(c.position_change());
                },
            )
            .await;
        }
    }
    if let Some(drag) = drag {
        (d.on_drag_start)(&down, &drag, over_slop.get());
        (d.on_drag)(&drag, over_slop.get());
        let up = drag_internal(
            s,
            drag.id,
            |c| {
                (d.on_drag)(c, c.position_change());
                c.consume();
            },
            None,
            |c| c.is_consumed(),
        )
        .await;
        match up {
            None => (d.on_drag_cancel)(),
            Some(up) => (d.on_drag_end)(&up),
        }
    }
}

/// `detect_drag_gestures_after_long_press`'s callbacks, then awaited (Compose's
/// `detectDragGesturesAfterLongPress`).
pub struct DetectDragGesturesAfterLongPress {
    scope: PointerInputScope,
    on_drag_start: Rc<dyn Fn(Offset)>,
    on_drag_end: Rc<dyn Fn()>,
    on_drag_cancel: Rc<dyn Fn()>,
    on_drag: DragCallback,
}

impl DetectDragGesturesAfterLongPress {
    pub(crate) fn new(scope: PointerInputScope, on_drag: impl Fn(&PointerInputChange, Offset) + 'static) -> Self {
        DetectDragGesturesAfterLongPress {
            scope,
            on_drag_start: Rc::new(|_| {}),
            on_drag_end: Rc::new(|| {}),
            on_drag_cancel: Rc::new(|| {}),
            on_drag: Rc::new(on_drag),
        }
    }

    /// The long press happened: where.
    pub fn on_drag_start(mut self, on_drag_start: impl Fn(Offset) + 'static) -> Self {
        self.on_drag_start = Rc::new(on_drag_start);
        self
    }

    pub fn on_drag_end(mut self, on_drag_end: impl Fn() + 'static) -> Self {
        self.on_drag_end = Rc::new(on_drag_end);
        self
    }

    /// The drag was taken by another, or the detector cancelled during it.
    pub fn on_drag_cancel(mut self, on_drag_cancel: impl Fn() + 'static) -> Self {
        self.on_drag_cancel = Rc::new(on_drag_cancel);
        self
    }
}

/// Calls the cancel callback if dropped (the coroutine cancelled) while armed (Compose's `catch
/// (CancellationException)`).
struct CancelOnDrop(Option<Rc<dyn Fn()>>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

impl IntoFuture for DetectDragGesturesAfterLongPress {
    type Output = ();
    type IntoFuture = Pin<Box<dyn Future<Output = ()>>>;

    fn into_future(self) -> Self::IntoFuture {
        let d = Rc::new(self);
        Box::pin(async move {
            let scope = d.scope.clone();
            await_each_gesture(&scope, move |s| {
                let d = d.clone();
                async move {
                    let mut guard = CancelOnDrop(Some(d.on_drag_cancel.clone()));
                    let down = await_first_down(&s, false, PointerEventPass::Main).await;
                    if let Some(drag) = await_long_press_or_cancellation(&s, down.id).await {
                        (d.on_drag_start)(drag.position);
                        let ended = drag_until_up(&s, drag.id, |c| {
                            (d.on_drag)(c, c.position_change());
                            c.consume();
                        })
                        .await;
                        guard.0 = None;
                        if ended {
                            s.current_event().changes.iter().filter(|c| c.changed_to_up()).for_each(|c| c.consume());
                            (d.on_drag_end)();
                        } else {
                            (d.on_drag_cancel)();
                        }
                    }
                    guard.0 = None;
                }
            })
            .await
        })
    }
}

/// `detect_vertical_drag_gestures`' and `detect_horizontal_drag_gestures`' callbacks, then awaited (Compose's
/// `detectVerticalDragGestures`, `detectHorizontalDragGestures`).
pub struct DetectAxisDragGestures {
    scope: PointerInputScope,
    orientation: Orientation,
    on_drag_start: Rc<dyn Fn(Offset)>,
    on_drag_end: Rc<dyn Fn()>,
    on_drag_cancel: Rc<dyn Fn()>,
    on_drag: Rc<dyn Fn(&PointerInputChange, f32)>,
}

impl DetectAxisDragGestures {
    pub(crate) fn new(
        scope: PointerInputScope,
        orientation: Orientation,
        on_drag: impl Fn(&PointerInputChange, f32) + 'static,
    ) -> Self {
        DetectAxisDragGestures {
            scope,
            orientation,
            on_drag_start: Rc::new(|_| {}),
            on_drag_end: Rc::new(|| {}),
            on_drag_cancel: Rc::new(|| {}),
            on_drag: Rc::new(on_drag),
        }
    }

    pub fn on_drag_start(mut self, on_drag_start: impl Fn(Offset) + 'static) -> Self {
        self.on_drag_start = Rc::new(on_drag_start);
        self
    }

    pub fn on_drag_end(mut self, on_drag_end: impl Fn() + 'static) -> Self {
        self.on_drag_end = Rc::new(on_drag_end);
        self
    }

    pub fn on_drag_cancel(mut self, on_drag_cancel: impl Fn() + 'static) -> Self {
        self.on_drag_cancel = Rc::new(on_drag_cancel);
        self
    }
}

impl IntoFuture for DetectAxisDragGestures {
    type Output = ();
    type IntoFuture = Pin<Box<dyn Future<Output = ()>>>;

    fn into_future(self) -> Self::IntoFuture {
        let d = Rc::new(self);
        Box::pin(async move {
            let scope = d.scope.clone();
            await_each_gesture(&scope, move |s| {
                let d = d.clone();
                async move {
                    let vertical = d.orientation == Orientation::Vertical;
                    let axis = move |o: Offset| if vertical { o.y } else { o.x };
                    let down = await_first_down(&s, false, PointerEventPass::Main).await;
                    let over_slop = std::cell::Cell::new(0.0f32);
                    let drag = await_pointer_slop_or_cancellation(
                        &s,
                        down.id,
                        down.r#type,
                        Some(d.orientation),
                        Offset::Zero,
                        |c, over| {
                            c.consume();
                            over_slop.set(axis(over));
                        },
                    )
                    .await;
                    if let Some(drag) = drag {
                        (d.on_drag_start)(drag.position);
                        (d.on_drag)(&drag, over_slop.get());
                        let ended = axis_drag(&s, drag.id, d.orientation, |c| {
                            (d.on_drag)(c, axis(c.position_change()));
                            c.consume();
                        })
                        .await;
                        if ended { (d.on_drag_end)() } else { (d.on_drag_cancel)() }
                    }
                }
            })
            .await
        })
    }
}
