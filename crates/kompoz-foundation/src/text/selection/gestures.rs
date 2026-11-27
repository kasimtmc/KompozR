//! Selection gestures of the mouse and of touch (Compose's SelectionGestures.kt): a mouse's clicks, double and
//! triple clicks and drags, shift to extend; a finger's long press then drag, double tap.

use super::adjustment::SelectionAdjustment;
use crate::gestures::{AwaitPointerEventScopeGestures, PointerInputScopeGestures, pointer_slop};
use crate::text::long_press_text_drag_observer::TextDragObserver;
use kompoz_graphics::Offset;
use kompoz_ui::Modifier;
use kompoz_ui::input::{
    AwaitPointerEventScope, PointerEvent, PointerEventPass, PointerInputChange, PointerInputScope, PointerType,
};
use kompoz_ui::platform::ViewConfiguration;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What a mouse's selection gestures tell (Compose's MouseSelectionObserver): without shift a selection starts
/// anew (a click sets both ends), with shift it extends (a click sets the end). Each returns whether the event is
/// taken.
pub trait MouseSelectionObserver {
    /// A click with shift.
    fn on_extend(&self, down_position: Offset) -> bool;
    /// A drag after a click with shift.
    fn on_extend_drag(&self, drag_position: Offset) -> bool;
    /// A click without shift, the `click_count`th in a row, adjusted by `adjustment`.
    fn on_start(&self, down_position: Offset, adjustment: SelectionAdjustment, click_count: u32) -> bool;
    /// A drag without shift.
    fn on_drag(&self, drag_position: Offset, adjustment: SelectionAdjustment) -> bool;
    /// The gesture ended.
    fn on_drag_done(&self);
}

/// The key of the touch mode's pointer input (Compose's STATIC_KEY: clashing with no other).
const STATIC_KEY: i32 = 867_5309;

/// Tells `update_touch_mode` whether each event is a finger's (true) or a mouse's (Compose's
/// `updateSelectionTouchMode`): seen first, in the Initial pass.
pub fn update_selection_touch_mode(modifier: Modifier, update_touch_mode: impl Fn(bool) + 'static) -> Modifier {
    let update = Rc::new(update_touch_mode);
    modifier.pointer_input(STATIC_KEY, move |scope: PointerInputScope| {
        let update = update.clone();
        async move {
            scope
                .await_pointer_event_scope(|s| async move {
                    loop {
                        let event = s.await_pointer_event_with(PointerEventPass::Initial).await;
                        update(!is_mouse_or_touch_pad(&event));
                    }
                })
                .await
        }
    })
}

/// Whether the event is a mouse's (or a touchpad's) (Compose's `isMouseOrTouchPad`).
pub fn is_mouse_or_touch_pad(event: &PointerEvent) -> bool {
    !event.changes.is_empty() && event.changes.iter().all(|c| c.r#type == PointerType::Mouse)
}

/// The adjustment of a first long press (Compose's FirstLongPressSelectionAdjustment, Android's): by word.
pub const FirstLongPressSelectionAdjustment: SelectionAdjustment = SelectionAdjustment::Word;

/// Runs `action` when dropped unless disarmed: what Kotlin's `catch (CancellationException)` and `finally` do
/// for a gesture cancelled where it waits.
struct OnDrop<F: FnMut()>(Option<F>);

impl<F: FnMut()> OnDrop<F> {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl<F: FnMut()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        if let Some(mut action) = self.0.take() {
            action();
        }
    }
}

/// The selection gestures of a mouse and of touch (Compose's `awaitSelectionGestures`): what the first down is
/// decides which. A mouse's primary button down, unconsumed, starts a mouse selection; a finger's first press
/// waits for a long press, its next ones (a double tap) select at once — if there is a `text_drag_observer` (none
/// where only a mouse selects, as between the texts of a container).
pub async fn await_selection_gestures(
    scope: PointerInputScope,
    mouse_selection_observer: Rc<dyn MouseSelectionObserver>,
    text_drag_observer: Option<Rc<dyn TextDragObserver>>,
) {
    let clicks_counter = Rc::new(RefCell::new(ClicksCounter::new(scope.view_configuration())));
    scope
        .await_each_gesture(|s| {
            let (mouse, touch, clicks_counter) =
                (mouse_selection_observer.clone(), text_drag_observer.clone(), clicks_counter.clone());
            async move {
                let down_event = await_down(&s).await;
                clicks_counter.borrow_mut().update(&down_event);
                let clicks = clicks_counter.borrow().clicks;
                let is_precise = is_mouse_or_touch_pad(&down_event);
                if is_precise
                    && down_event.buttons.is_primary_pressed()
                    && down_event.changes.iter().all(|c| !c.is_consumed())
                {
                    mouse_selection(&s, mouse, clicks, &down_event).await;
                } else if !is_precise && let Some(observer) = touch {
                    if clicks == 1 {
                        touch_selection_first_press(&s, observer, &down_event).await;
                    } else {
                        touch_selection_subsequent_press(&s, observer, &down_event, clicks).await;
                    }
                }
            }
        })
        .await
}

/// A finger's first press (Compose's `touchSelectionFirstPress`): a long press (not moved away) starts the
/// selection, then drags move its end; without a long press nothing is told.
pub async fn touch_selection_first_press(
    s: &AwaitPointerEventScope,
    observer: Rc<dyn TextDragObserver>,
    down_event: &PointerEvent,
) {
    let first_down = down_event.changes[0].clone();
    let cancelled = observer.clone();
    let mut guard = OnDrop(Some(move || cancelled.on_cancel()));
    let long_press = s.await_long_press_or_cancellation(first_down.id).await;
    let Some(long_press) = long_press.filter(|lp| distance_is_tolerable(&s.view_configuration(), &first_down, lp))
    else {
        guard.disarm();
        return;
    };
    observer.on_start(long_press.position, FirstLongPressSelectionAdjustment);
    let drag_observer = observer.clone();
    let drag_completed_with_up = s
        .drag(long_press.id, move |change| {
            drag_observer.on_drag(change.position_change());
            change.consume();
        })
        .await;
    guard.disarm();
    if drag_completed_with_up {
        // The up of a drag that ended well is taken.
        s.current_event().changes.iter().filter(|c| c.changed_to_up()).for_each(|c| c.consume());
        observer.on_stop();
    } else {
        observer.on_cancel();
    }
}

/// How a subsequent press ended before dragging (Compose's DownResolution).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DownResolution {
    Up,
    Drag,
    Timeout,
    Cancel,
}

/// A finger's press after the first (Compose's `touchSelectionSubsequentPress`): the selection starts at once
/// (a word on the second, a paragraph from the third) and drags move its end.
async fn touch_selection_subsequent_press(
    s: &AwaitPointerEventScope,
    observer: Rc<dyn TextDragObserver>,
    down_event: &PointerEvent,
    clicks: u32,
) {
    let first_down = down_event.changes[0].clone();
    let pointer_id = first_down.id;
    let cancelled = observer.clone();
    let mut guard = OnDrop(Some(move || cancelled.on_cancel()));
    // With a subsequent press a selection always starts.
    observer.on_start(
        first_down.position,
        if clicks > 2 { SelectionAdjustment::Paragraph } else { SelectionAdjustment::Word },
    );
    let over_slop = Rc::new(Cell::new(Offset::Unspecified));
    let slop = over_slop.clone();
    let timeout = s.view_configuration().long_press_timeout_millis();
    let down_resolution = s
        .with_timeout_or_null(timeout, move |s| async move {
            let first_drag_past_slop = s
                .await_touch_slop_or_cancellation(pointer_id, |change, over| {
                    change.consume();
                    slop.set(over);
                })
                .await;
            // Past the slop: a drag started.
            if first_drag_past_slop.is_some() && slop.get().is_specified() {
                return DownResolution::Drag;
            }
            // Else it was cancelled, or the pointer is up.
            let current_change = s.current_event().changes[0].clone();
            if current_change.changed_to_up_ignore_consumed() {
                current_change.consume();
                DownResolution::Up
            } else {
                DownResolution::Cancel
            }
        })
        .await
        .unwrap_or(DownResolution::Timeout);
    match down_resolution {
        DownResolution::Cancel => {
            guard.disarm();
            observer.on_cancel();
            return;
        }
        DownResolution::Up => {
            // A tap: the selection started stays.
            guard.disarm();
            observer.on_stop();
            return;
        }
        // A drag began: the slop's leftover first.
        DownResolution::Drag => observer.on_drag(over_slop.get()),
        // A long press: drags from here.
        DownResolution::Timeout => {}
    }
    let drag_observer = observer.clone();
    let drag_completed_with_up = s
        .drag(pointer_id, move |change| {
            drag_observer.on_drag(change.position_change());
            change.consume();
        })
        .await;
    guard.disarm();
    if drag_completed_with_up {
        s.current_event().changes.iter().filter(|c| c.changed_to_up()).for_each(|c| c.consume());
        observer.on_stop();
    } else {
        observer.on_cancel();
    }
}

/// A mouse's selection gesture (Compose's `mouseSelection`): with shift the click extends the selection and drags
/// extend it on; without, the click starts one (one click as it is, two by word, three by paragraph) that drags
/// extend by the same unit.
pub async fn mouse_selection(
    s: &AwaitPointerEventScope,
    observer: Rc<dyn MouseSelectionObserver>,
    clicks: u32,
    down: &PointerEvent,
) {
    let down_change = down.changes[0].clone();
    let done = observer.clone();
    if down.keyboard_modifiers.is_shift_pressed() {
        if observer.on_extend(down_change.position) {
            let _done = OnDrop(Some(move || done.on_drag_done()));
            down_change.consume();
            let drag_observer = observer.clone();
            let should_consume_up = s
                .drag(down_change.id, move |c| {
                    if drag_observer.on_extend_drag(c.position) {
                        c.consume();
                    }
                })
                .await;
            if should_consume_up {
                s.current_event().changes.iter().filter(|c| c.changed_to_up()).for_each(|c| c.consume());
            }
        }
    } else {
        let selection_adjustment = match clicks {
            1 => SelectionAdjustment::None,
            2 => SelectionAdjustment::Word,
            _ => SelectionAdjustment::Paragraph,
        };
        if observer.on_start(down_change.position, selection_adjustment, clicks) {
            let _done = OnDrop(Some(move || done.on_drag_done()));
            let drag_consumed = Rc::new(Cell::new(selection_adjustment != SelectionAdjustment::None));
            let (consumed, drag_observer) = (drag_consumed.clone(), observer.clone());
            let should_consume_up = s
                .drag(down_change.id, move |c| {
                    if drag_observer.on_drag(c.position, selection_adjustment) {
                        c.consume();
                        consumed.set(true);
                    }
                })
                .await;
            if should_consume_up && drag_consumed.get() {
                s.current_event().changes.iter().filter(|c| c.changed_to_up()).for_each(|c| c.consume());
            }
        }
    }
}

/// Counts clicks in a row (Compose's ClicksCounter): a press within the double tap timeout of the last one, and
/// near it, is one more.
pub struct ClicksCounter {
    view_configuration: ViewConfiguration,
    pub clicks: u32,
    prev_click: Option<PointerInputChange>,
}

impl ClicksCounter {
    pub fn new(view_configuration: ViewConfiguration) -> ClicksCounter {
        ClicksCounter { view_configuration, clicks: 0, prev_click: None }
    }

    /// Counts the press of `event`.
    pub fn update(&mut self, event: &PointerEvent) {
        let new_click = event.changes[0].clone();
        match &self.prev_click {
            Some(prev) if self.time_is_tolerable(prev, &new_click) && self.position_is_tolerable(prev, &new_click) => {
                self.clicks += 1;
            }
            _ => self.clicks = 1,
        }
        self.prev_click = Some(new_click);
    }

    fn time_is_tolerable(&self, prev_click: &PointerInputChange, new_click: &PointerInputChange) -> bool {
        new_click.uptime_millis - prev_click.uptime_millis < self.view_configuration.double_tap_timeout_millis()
    }

    /// Near it: within the system's double click area around it where it says one (KompozR's: Windows'), else within
    /// the pointer's slop (Compose's).
    fn position_is_tolerable(&self, prev_click: &PointerInputChange, new_click: &PointerInputChange) -> bool {
        match self.view_configuration.double_click_area {
            Some(area) => {
                let d = new_click.position - prev_click.position;
                d.x.abs() <= area.width / 2.0 && d.y.abs() <= area.height / 2.0
            }
            None => distance_is_tolerable(&self.view_configuration, prev_click, new_click),
        }
    }
}

/// The next event with every pointer gone down (Compose's `awaitDown`).
async fn await_down(s: &AwaitPointerEventScope) -> PointerEvent {
    loop {
        let event = s.await_pointer_event_with(PointerEventPass::Main).await;
        if event.changes.iter().all(|c| c.changed_to_down()) {
            return event;
        }
    }
}

/// Whether two changes are within the slop of each other (Compose's `distanceIsTolerable`).
fn distance_is_tolerable(
    view_configuration: &ViewConfiguration,
    change1: &PointerInputChange,
    change2: &PointerInputChange,
) -> bool {
    let slop = pointer_slop(view_configuration, change1.r#type);
    (change1.position - change2.position).get_distance() < slop
}
