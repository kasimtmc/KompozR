//! The sliders' states, as Compose's Slider.kt has them: [`SliderState`] (a draggable state: the value, its range and
//! steps, the callbacks, where the thumb was pressed and dragged to), [`RangeSliderState`] (the active range's two
//! ends), their remembered forms, the deprecated [`SliderPositions`], and the arithmetic they share (the ticks of
//! the steps, snapping to the nearest, scaling between a value range and pixels).

// (the deprecated SliderPositions and its track are defined here, as Compose's are)
#![allow(deprecated)]

use kompoz_animation::{MutatePriority, MutatorMutex};
use kompoz_foundation::gestures::{DragBlock, DragScope, DraggableState, LocalFuture, Orientation};
use kompoz_graphics::{Offset, lerp};
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{
    MutableState, SavedValue, Saver, kompozable, list_saver, mutable_state_of, named, named_args,
    remember_saveable_with,
};
use kompoz_ui::util::{pack_floats, unpack_float1, unpack_float2};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::ops::RangeInclusive;
use std::rc::Rc;

// ---------------------------------------------------------------------------------------------------------------
// SliderState

/// The state of a [`Slider`](crate::StateSlider) (Compose's SliderState): its `value` in `value_range` — snapped to
/// the nearest of `steps` evenly spread values between the range's ends (none: any value) unless
/// [`should_auto_snap`](SliderState::should_auto_snap) is off —, the callback told each new value
/// ([`on_value_change`](SliderState::on_value_change): without one the state takes the value itself) and the one
/// told when a change ends ([`on_value_change_finished`](SliderState::on_value_change_finished)). It is the
/// slider's [`DraggableState`]: a drag of the thumb moves the value. A cheap handle: its clones are the same state.
///
/// Make one with [`SliderState()`](fn@SliderState) or remember one with [`remember_slider_state`].
#[derive(Clone)]
pub struct SliderState {
    inner: Rc<SliderStateInner>,
}

struct SliderStateInner {
    steps: i32,
    on_value_change_finished: RefCell<Option<Rc<dyn Fn()>>>,
    value_range: RangeInclusive<f32>,
    value_state: MutableState<f32>,
    on_value_change: RefCell<Option<Rc<dyn Fn(f32)>>>,
    should_auto_snap: Cell<bool>,
    tick_fractions: Vec<f32>,
    total_width: MutableState<i32>,
    total_height: MutableState<i32>,
    is_rtl: Cell<bool>,
    thumb_width: MutableState<i32>,
    thumb_height: MutableState<i32>,
    orientation: Cell<Orientation>,
    reverse_vertical_direction: Cell<bool>,
    is_dragging: MutableState<bool>,
    raw_offset: MutableState<f32>,
    press_offset: MutableState<f32>,
    scroll_mutex: MutatorMutex,
}

impl PartialEq for SliderState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for SliderState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SliderState(value={}, steps={}, valueRange={:?})", self.value(), self.steps(), self.value_range())
    }
}

/// A slider's state (Compose's `SliderState(value, steps, onValueChangeFinished, valueRange)`): `value` (taken as
/// it is: it isn't snapped to the steps until it is set again), `steps` (at least 0) between `value_range`'s ends,
/// `on_value_change_finished` told when a change ends.
#[named(type)]
pub fn SliderState(
    #[default(0.0)] value: f32,
    #[default(0)] steps: i32,
    #[default(None)] on_value_change_finished: Option<Rc<dyn Fn()>>,
    #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
) -> SliderState {
    SliderState::new(value, steps, on_value_change_finished, value_range)
}

impl SliderState {
    /// [`SliderState()`](fn@SliderState) with all its parameters, for plain code.
    pub fn new(
        value: f32,
        steps: i32,
        on_value_change_finished: Option<Rc<dyn Fn()>>,
        value_range: RangeInclusive<f32>,
    ) -> SliderState {
        let tick_fractions = steps_to_tick_fractions(steps);
        let raw_offset = scale(*value_range.start(), *value_range.end(), value, 0.0, 0.0);
        SliderState {
            inner: Rc::new(SliderStateInner {
                steps,
                on_value_change_finished: RefCell::new(on_value_change_finished),
                value_range,
                value_state: mutable_state_of(value),
                on_value_change: RefCell::new(None),
                should_auto_snap: Cell::new(true),
                tick_fractions,
                total_width: mutable_state_of(0),
                total_height: mutable_state_of(0),
                is_rtl: Cell::new(false),
                thumb_width: mutable_state_of(0),
                thumb_height: mutable_state_of(0),
                orientation: Cell::new(Orientation::Horizontal),
                reverse_vertical_direction: Cell::new(false),
                is_dragging: mutable_state_of(false),
                raw_offset: mutable_state_of(raw_offset),
                press_offset: mutable_state_of(0.0),
                scroll_mutex: MutatorMutex::new(),
            }),
        }
    }
}

impl SliderState {
    /// The value (Compose's `value`): read, it is observed.
    pub fn value(&self) -> f32 {
        self.inner.value_state.get()
    }

    /// Sets the value (Compose's `value =`): coerced into the range and snapped to the nearest step unless
    /// [`should_auto_snap`](SliderState::should_auto_snap) is off.
    pub fn set_value(&self, new_val: f32) {
        let value = if self.inner.should_auto_snap.get() { self.calculate_snapped_value(new_val) } else { new_val };
        self.inner.value_state.set(value);
    }

    fn calculate_snapped_value(&self, new_val: f32) -> f32 {
        let (start, end) = (*self.inner.value_range.start(), *self.inner.value_range.end());
        let coerced_value = coerce_in(new_val, start, end);
        snap_value_to_tick(coerced_value, &self.inner.tick_fractions, start, end)
    }

    /// The number of values between the range's ends the value snaps to (Compose's `steps`).
    pub fn steps(&self) -> i32 {
        self.inner.steps
    }

    /// The range of the values (Compose's `valueRange`).
    pub fn value_range(&self) -> RangeInclusive<f32> {
        self.inner.value_range.clone()
    }

    /// What a change's end calls (Compose's `onValueChangeFinished`).
    pub fn on_value_change_finished(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_value_change_finished.borrow().clone()
    }

    /// Sets what a change's end calls (Compose's `onValueChangeFinished =`).
    pub fn set_on_value_change_finished(&self, on_value_change_finished: Option<Rc<dyn Fn()>>) {
        *self.inner.on_value_change_finished.borrow_mut() = on_value_change_finished;
    }

    /// What a new value from the user is given to (Compose's `onValueChange`): none, the state takes it itself.
    pub fn on_value_change(&self) -> Option<Rc<dyn Fn(f32)>> {
        self.inner.on_value_change.borrow().clone()
    }

    /// Sets what a new value from the user is given to (Compose's `onValueChange =`).
    pub fn set_on_value_change(&self, on_value_change: Option<Rc<dyn Fn(f32)>>) {
        *self.inner.on_value_change.borrow_mut() = on_value_change;
    }

    /// Whether a value set is snapped to the steps (Compose's `shouldAutoSnap`, true at first): off, a value set is
    /// kept as it is (to animate it to a step, say).
    pub fn should_auto_snap(&self) -> bool {
        self.inner.should_auto_snap.get()
    }

    /// Sets whether a value set is snapped to the steps (Compose's `shouldAutoSnap =`).
    pub fn set_should_auto_snap(&self, should_auto_snap: bool) {
        self.inner.should_auto_snap.set(should_auto_snap);
    }

    /// The value as a fraction of the range, 0 to 1 (Compose's `coercedValueAsFraction`).
    pub fn coerced_value_as_fraction(&self) -> f32 {
        let (start, end) = (*self.inner.value_range.start(), *self.inner.value_range.end());
        calc_fraction(start, end, coerce_in(self.value(), start, end))
    }

    /// Whether the thumb is being dragged (Compose's `isDragging`): read, it is observed.
    pub fn is_dragging(&self) -> bool {
        self.inner.is_dragging.get()
    }

    /// Its saver (Compose's `SliderState.Saver(onValueChangeFinished, valueRange)`): the value and the steps; what
    /// isn't saved is given.
    pub fn Saver(
        on_value_change_finished: Option<Rc<dyn Fn()>>,
        value_range: RangeInclusive<f32>,
    ) -> Saver<SliderState> {
        list_saver(
            |it: &SliderState| vec![SavedValue::from(it.value()), SavedValue::from(it.steps() as i64)],
            move |it| {
                Some(SliderState::new(
                    it.first()?.as_f64()? as f32,
                    it.get(1)?.as_int()? as i32,
                    on_value_change_finished.clone(),
                    value_range.clone(),
                ))
            },
        )
    }

    // ---- what the slider reads and sets (Compose's internal members) ----

    pub(crate) fn tick_fractions(&self) -> &[f32] {
        &self.inner.tick_fractions
    }

    pub(crate) fn is_rtl(&self) -> bool {
        self.inner.is_rtl.get()
    }

    pub(crate) fn set_is_rtl(&self, is_rtl: bool) {
        self.inner.is_rtl.set(is_rtl);
    }

    pub(crate) fn thumb_width(&self) -> i32 {
        self.inner.thumb_width.get()
    }

    pub(crate) fn thumb_height(&self) -> i32 {
        self.inner.thumb_height.get()
    }

    pub(crate) fn set_thumb_size(&self, width: i32, height: i32) {
        self.inner.thumb_width.set(width);
        self.inner.thumb_height.set(height);
    }

    pub(crate) fn orientation(&self) -> Orientation {
        self.inner.orientation.get()
    }

    pub(crate) fn set_orientation(&self, orientation: Orientation) {
        self.inner.orientation.set(orientation);
    }

    pub(crate) fn reverse_vertical_direction(&self) -> bool {
        self.inner.reverse_vertical_direction.get()
    }

    pub(crate) fn set_reverse_vertical_direction(&self, reverse: bool) {
        self.inner.reverse_vertical_direction.set(reverse);
    }

    /// Compose's `updateDimensions`.
    pub(crate) fn update_dimensions(&self, new_total_width: i32, new_total_height: i32) {
        self.inner.total_width.set(new_total_width);
        self.inner.total_height.set(new_total_height);
    }

    /// The end of a gesture (Compose's `gestureEndAction`): `on_value_change_finished`, unless a drag is still on
    /// (a touch turned into a drag).
    pub(crate) fn gesture_end_action(&self) {
        if !self.inner.is_dragging.peek() {
            // check isDragging in case the change is still in progress (touch -> drag case)
            if let Some(f) = self.on_value_change_finished() {
                f();
            }
        }
    }

    /// A press at `pos` (Compose's `onPress`): the next delta moves the thumb there.
    pub(crate) fn on_press(&self, pos: Offset) {
        let i = &self.inner;
        let to = if i.orientation.get() == Orientation::Vertical {
            if i.reverse_vertical_direction.get() { i.total_height.peek() as f32 - pos.y } else { pos.y }
        } else if i.is_rtl.get() {
            i.total_width.peek() as f32 - pos.x
        } else {
            pos.x
        };
        i.press_offset.set(to - i.raw_offset.peek());
    }

    fn scale_to_user_value(&self, min_px: f32, max_px: f32, offset: f32) -> f32 {
        scale(min_px, max_px, offset, *self.inner.value_range.start(), *self.inner.value_range.end())
    }
}

/// Puts `is_dragging` back however the drag ends (Compose's `finally { isDragging = false }`).
struct DraggingFlag(MutableState<bool>);

impl Drop for DraggingFlag {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// The drag scope of a slider's state (Compose's `dragScope` object): each delta is dispatched.
struct SliderDragScope(SliderState);

impl DragScope for SliderDragScope {
    fn drag_by(&self, pixels: f32) {
        self.0.dispatch_raw_delta(pixels);
    }
}

impl DraggableState for SliderState {
    fn drag_block(&self, drag_priority: MutatePriority, block: DragBlock) -> LocalFuture<'_, ()> {
        let this = self.clone();
        // (Compose's `coroutineScope { … }` around it only scopes the block: nothing of its own here)
        Box::pin(async move {
            this.inner.is_dragging.set(true);
            let _finally = DraggingFlag(this.inner.is_dragging.clone());
            let scope: Rc<dyn DragScope> = Rc::new(SliderDragScope(this.clone()));
            named_args!(this.inner.scroll_mutex.mutate_with(scope, drag_priority, block)).await;
        })
    }

    fn dispatch_raw_delta(&self, delta: f32) {
        let i = &self.inner;
        let (max_px, min_px) = if i.orientation.get() == Orientation::Vertical {
            let max_px = (i.total_height.peek() as f32 - i.thumb_height.peek() as f32 / 2.0).max(0.0);
            (max_px, (i.thumb_height.peek() as f32 / 2.0).min(max_px))
        } else {
            let max_px = (i.total_width.peek() as f32 - i.thumb_width.peek() as f32 / 2.0).max(0.0);
            (max_px, (i.thumb_width.peek() as f32 / 2.0).min(max_px))
        };
        i.raw_offset.set(i.raw_offset.peek() + delta + i.press_offset.peek());
        i.press_offset.set(0.0);
        let offset_in_track = snap_value_to_tick(i.raw_offset.peek(), &i.tick_fractions, min_px, max_px);
        let scaled_user_value = self.scale_to_user_value(min_px, max_px, offset_in_track);
        if scaled_user_value != i.value_state.peek() {
            match self.on_value_change() {
                Some(on_value_change) => on_value_change(scaled_user_value),
                None => self.set_value(scaled_user_value),
            }
        }
    }
}

/// A [`SliderState()`](fn@SliderState) remembered where it is called, and saved (its value and steps) as the UI's
/// state is (Compose's `rememberSliderState(value, steps, onValueChangeFinished, valueRange)`).
#[kompozable]
pub fn remember_slider_state(
    #[default(0.0)] value: f32,
    #[default(0)] steps: i32,
    #[default(None)] on_value_change_finished: Option<Rc<dyn Fn()>>,
    #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
) -> SliderState {
    let saver = SliderState::Saver(on_value_change_finished.clone(), value_range.clone());
    remember_saveable_with(&saver, move || SliderState::new(value, steps, on_value_change_finished, value_range))
}

// ---------------------------------------------------------------------------------------------------------------
// RangeSliderState

/// The state of a [`RangeSlider`](crate::StateRangeSlider) (Compose's RangeSliderState): its active range's ends,
/// `active_range_start` ≤ `active_range_end`, in `value_range`, each snapped to the nearest of `steps` evenly spread
/// values between the range's ends (none: any value); the callback told when a change ends. A cheap handle: its
/// clones are the same state.
///
/// Make one with [`RangeSliderState()`](fn@RangeSliderState) or remember one with [`remember_range_slider_state`].
#[derive(Clone)]
pub struct RangeSliderState {
    inner: Rc<RangeSliderStateInner>,
}

struct RangeSliderStateInner {
    steps: i32,
    on_value_change_finished: RefCell<Option<Rc<dyn Fn()>>>,
    value_range: RangeInclusive<f32>,
    active_range_start_state: MutableState<f32>,
    active_range_end_state: MutableState<f32>,
    on_value_change: RefCell<Option<Rc<dyn Fn(SliderRange)>>>,
    tick_fractions: Vec<f32>,
    start_thumb_width: MutableState<f32>,
    start_thumb_height: MutableState<f32>,
    end_thumb_width: MutableState<f32>,
    end_thumb_height: MutableState<f32>,
    total_width: MutableState<i32>,
    raw_offset_start: MutableState<f32>,
    raw_offset_end: MutableState<f32>,
    is_dragging: MutableState<bool>,
    is_rtl: MutableState<bool>,
    max_px: MutableState<f32>,
    min_px: MutableState<f32>,
}

impl PartialEq for RangeSliderState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for RangeSliderState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RangeSliderState(activeRangeStart={}, activeRangeEnd={}, steps={}, valueRange={:?})",
            self.active_range_start(),
            self.active_range_end(),
            self.steps(),
            self.value_range()
        )
    }
}

/// A range slider's state (Compose's `RangeSliderState(activeRangeStart, activeRangeEnd, steps,
/// onValueChangeFinished, valueRange)`): the ends coerced into `value_range` and put in order (not snapped to the
/// steps until they are set again).
#[named(type)]
pub fn RangeSliderState(
    #[default(0.0)] active_range_start: f32,
    #[default(1.0)] active_range_end: f32,
    #[default(0)] steps: i32,
    #[default(None)] on_value_change_finished: Option<Rc<dyn Fn()>>,
    #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
) -> RangeSliderState {
    RangeSliderState::new(active_range_start, active_range_end, steps, on_value_change_finished, value_range)
}

impl RangeSliderState {
    /// [`RangeSliderState()`](fn@RangeSliderState) with all its parameters, for plain code.
    pub fn new(
        active_range_start: f32,
        active_range_end: f32,
        steps: i32,
        on_value_change_finished: Option<Rc<dyn Fn()>>,
        value_range: RangeInclusive<f32>,
    ) -> RangeSliderState {
        let (start, end) = (*value_range.start(), *value_range.end());
        let coerced_start = coerce_in(active_range_start, start, end);
        let coerced_end = coerce_in(active_range_end, start, end);
        RangeSliderState {
            inner: Rc::new(RangeSliderStateInner {
                steps,
                on_value_change_finished: RefCell::new(on_value_change_finished),
                value_range,
                active_range_start_state: mutable_state_of(coerced_start.min(coerced_end)),
                active_range_end_state: mutable_state_of(coerced_start.max(coerced_end)),
                on_value_change: RefCell::new(None),
                tick_fractions: steps_to_tick_fractions(steps),
                start_thumb_width: mutable_state_of(0.0),
                start_thumb_height: mutable_state_of(0.0),
                end_thumb_width: mutable_state_of(0.0),
                end_thumb_height: mutable_state_of(0.0),
                total_width: mutable_state_of(0),
                raw_offset_start: mutable_state_of(0.0),
                raw_offset_end: mutable_state_of(0.0),
                is_dragging: mutable_state_of(false),
                is_rtl: mutable_state_of(false),
                max_px: mutable_state_of(0.0),
                min_px: mutable_state_of(0.0),
            }),
        }
    }
}

impl RangeSliderState {
    /// The active range's start (Compose's `activeRangeStart`): read, it is observed.
    pub fn active_range_start(&self) -> f32 {
        self.inner.active_range_start_state.get()
    }

    /// Sets the active range's start (Compose's `activeRangeStart =`): coerced between the range's start and the
    /// active range's end, snapped to the nearest step.
    pub fn set_active_range_start(&self, new_val: f32) {
        let coerced_value = coerce_in(new_val, *self.inner.value_range.start(), self.active_range_end());
        let snapped_value = snap_value_to_tick(
            coerced_value,
            &self.inner.tick_fractions,
            *self.inner.value_range.start(),
            *self.inner.value_range.end(),
        );
        self.inner.active_range_start_state.set(snapped_value);
    }

    /// The active range's end (Compose's `activeRangeEnd`): read, it is observed.
    pub fn active_range_end(&self) -> f32 {
        self.inner.active_range_end_state.get()
    }

    /// Sets the active range's end (Compose's `activeRangeEnd =`): coerced between the active range's start and the
    /// range's end, snapped to the nearest step.
    pub fn set_active_range_end(&self, new_val: f32) {
        let coerced_value = coerce_in(new_val, self.active_range_start(), *self.inner.value_range.end());
        let snapped_value = snap_value_to_tick(
            coerced_value,
            &self.inner.tick_fractions,
            *self.inner.value_range.start(),
            *self.inner.value_range.end(),
        );
        self.inner.active_range_end_state.set(snapped_value);
    }

    /// The number of values between the range's ends the ends snap to (Compose's `steps`).
    pub fn steps(&self) -> i32 {
        self.inner.steps
    }

    /// The range of the values (Compose's `valueRange`).
    pub fn value_range(&self) -> RangeInclusive<f32> {
        self.inner.value_range.clone()
    }

    /// What a change's end calls (Compose's `onValueChangeFinished`).
    pub fn on_value_change_finished(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_value_change_finished.borrow().clone()
    }

    /// Sets what a change's end calls (Compose's `onValueChangeFinished =`).
    pub fn set_on_value_change_finished(&self, on_value_change_finished: Option<Rc<dyn Fn()>>) {
        *self.inner.on_value_change_finished.borrow_mut() = on_value_change_finished;
    }

    /// Its saver (Compose's `RangeSliderState.Saver(onValueChangeFinished, valueRange)`): the ends and the steps.
    pub fn Saver(
        on_value_change_finished: Option<Rc<dyn Fn()>>,
        value_range: RangeInclusive<f32>,
    ) -> Saver<RangeSliderState> {
        list_saver(
            |it: &RangeSliderState| {
                vec![
                    SavedValue::from(it.active_range_start()),
                    SavedValue::from(it.active_range_end()),
                    SavedValue::from(it.steps() as i64),
                ]
            },
            move |it| {
                Some(RangeSliderState::new(
                    it.first()?.as_f64()? as f32,
                    it.get(1)?.as_f64()? as f32,
                    it.get(2)?.as_int()? as i32,
                    on_value_change_finished.clone(),
                    value_range.clone(),
                ))
            },
        )
    }

    // ---- what the slider reads and sets (Compose's internal members) ----

    pub(crate) fn on_value_change(&self) -> Option<Rc<dyn Fn(SliderRange)>> {
        self.inner.on_value_change.borrow().clone()
    }

    pub(crate) fn set_on_value_change(&self, on_value_change: Option<Rc<dyn Fn(SliderRange)>>) {
        *self.inner.on_value_change.borrow_mut() = on_value_change;
    }

    pub(crate) fn tick_fractions(&self) -> &[f32] {
        &self.inner.tick_fractions
    }

    pub(crate) fn start_thumb_width(&self) -> f32 {
        self.inner.start_thumb_width.get()
    }

    pub(crate) fn start_thumb_height(&self) -> f32 {
        self.inner.start_thumb_height.get()
    }

    pub(crate) fn end_thumb_width(&self) -> f32 {
        self.inner.end_thumb_width.get()
    }

    pub(crate) fn end_thumb_height(&self) -> f32 {
        self.inner.end_thumb_height.get()
    }

    pub(crate) fn set_start_thumb_size(&self, width: f32, height: f32) {
        self.inner.start_thumb_width.set(width);
        self.inner.start_thumb_height.set(height);
    }

    pub(crate) fn set_end_thumb_size(&self, width: f32, height: f32) {
        self.inner.end_thumb_width.set(width);
        self.inner.end_thumb_height.set(height);
    }

    pub(crate) fn total_width(&self) -> i32 {
        self.inner.total_width.peek()
    }

    pub(crate) fn set_total_width(&self, total_width: i32) {
        self.inner.total_width.set(total_width);
    }

    pub(crate) fn raw_offset_start(&self) -> f32 {
        self.inner.raw_offset_start.peek()
    }

    pub(crate) fn raw_offset_end(&self) -> f32 {
        self.inner.raw_offset_end.peek()
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.inner.is_dragging.peek()
    }

    pub(crate) fn set_is_dragging(&self, is_dragging: bool) {
        self.inner.is_dragging.set(is_dragging);
    }

    pub(crate) fn is_rtl(&self) -> bool {
        self.inner.is_rtl.peek()
    }

    pub(crate) fn set_is_rtl(&self, is_rtl: bool) {
        self.inner.is_rtl.set(is_rtl);
    }

    /// The end of a gesture (Compose's `gestureEndAction`): `on_value_change_finished`.
    pub(crate) fn gesture_end_action(&self, _dragging_start: bool) {
        if let Some(f) = self.on_value_change_finished() {
            f();
        }
    }

    /// A drag of the start or the end thumb by `offset` px (Compose's `onDrag`).
    pub(crate) fn on_drag(&self, is_start: bool, offset: f32) {
        let i = &self.inner;
        let (min_px, max_px) = (i.min_px.peek(), i.max_px.peek());
        let offset_range = if is_start {
            i.raw_offset_start.set(i.raw_offset_start.peek() + offset);
            i.raw_offset_end.set(self.scale_to_offset(min_px, max_px, i.active_range_end_state.peek()));
            let offset_end = i.raw_offset_end.peek();
            let mut offset_start = coerce_in(i.raw_offset_start.peek(), min_px, offset_end);
            offset_start = snap_value_to_tick(offset_start, &i.tick_fractions, min_px, max_px);
            SliderRange::new(offset_start.min(offset_end), offset_end)
        } else {
            i.raw_offset_end.set(i.raw_offset_end.peek() + offset);
            i.raw_offset_start.set(self.scale_to_offset(min_px, max_px, i.active_range_start_state.peek()));
            let offset_start = i.raw_offset_start.peek();
            let mut offset_end = coerce_in(i.raw_offset_end.peek(), offset_start, max_px);
            offset_end = snap_value_to_tick(offset_end, &i.tick_fractions, min_px, max_px);
            SliderRange::new(offset_start, offset_end.max(offset_start))
        };
        let scaled_user_value = self.scale_to_user_value(is_start, min_px, max_px, offset_range);
        if scaled_user_value != SliderRange::new(i.active_range_start_state.peek(), i.active_range_end_state.peek()) {
            match self.on_value_change() {
                Some(on_value_change) => on_value_change(scaled_user_value),
                None => {
                    self.set_active_range_start(scaled_user_value.start());
                    self.set_active_range_end(scaled_user_value.end_inclusive());
                }
            }
        }
    }

    /// The start as a fraction of the range (Compose's `coercedActiveRangeStartAsFraction`).
    pub(crate) fn coerced_active_range_start_as_fraction(&self) -> f32 {
        calc_fraction(*self.inner.value_range.start(), *self.inner.value_range.end(), self.active_range_start())
    }

    /// The end as a fraction of the range (Compose's `coercedActiveRangeEndAsFraction`).
    pub(crate) fn coerced_active_range_end_as_fraction(&self) -> f32 {
        calc_fraction(*self.inner.value_range.start(), *self.inner.value_range.end(), self.active_range_end())
    }

    /// The steps before the end thumb (Compose's `startSteps`).
    pub(crate) fn start_steps(&self) -> i32 {
        (self.inner.steps as f32 * self.coerced_active_range_end_as_fraction()).floor() as i32
    }

    /// The steps after the start thumb (Compose's `endSteps`).
    pub(crate) fn end_steps(&self) -> i32 {
        (self.inner.steps as f32 * (1.0 - self.coerced_active_range_start_as_fraction())).floor() as i32
    }

    // scales range offset from within minPx..maxPx to within valueRange.start..valueRange.end
    fn scale_to_user_value(&self, is_start: bool, min_px: f32, max_px: f32, offset: SliderRange) -> SliderRange {
        scale_range(is_start, min_px, max_px, offset, *self.inner.value_range.start(), *self.inner.value_range.end())
    }

    // scales float userValue within valueRange.start..valueRange.end to within minPx..maxPx
    fn scale_to_offset(&self, min_px: f32, max_px: f32, user_value: f32) -> f32 {
        scale(*self.inner.value_range.start(), *self.inner.value_range.end(), user_value, min_px, max_px)
    }

    /// The thumbs' span in px, and their places, from the latest measure (Compose's `updateMinMaxPx`): kept while
    /// dragged. Its reads are observed as Compose's are (the measure calling it runs again as a drag ends or a thumb
    /// changes its size, so the places follow the value then).
    pub(crate) fn update_min_max_px(&self) {
        let i = &self.inner;
        let new_max_px = (i.total_width.get() as f32 - i.end_thumb_width.get() / 2.0).max(0.0);
        let new_min_px = (i.start_thumb_width.get() / 2.0).min(new_max_px);
        let (start, end) = (i.active_range_start_state.get(), i.active_range_end_state.get());
        if !i.is_dragging.get() && (i.min_px.get() != new_min_px || i.max_px.get() != new_max_px || start != end) {
            i.min_px.set(new_min_px);
            i.max_px.set(new_max_px);
            i.raw_offset_start.set(self.scale_to_offset(new_min_px, new_max_px, start));
            i.raw_offset_end.set(self.scale_to_offset(new_min_px, new_max_px, end));
        }
    }
}

/// A [`RangeSliderState()`](fn@RangeSliderState) remembered where it is called, and saved (its ends and steps) as
/// the UI's state is (Compose's `rememberRangeSliderState(activeRangeStart, activeRangeEnd, steps,
/// onValueChangeFinished, valueRange)`).
#[kompozable]
pub fn remember_range_slider_state(
    #[default(0.0)] active_range_start: f32,
    #[default(1.0)] active_range_end: f32,
    #[default(0)] steps: i32,
    #[default(None)] on_value_change_finished: Option<Rc<dyn Fn()>>,
    #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
) -> RangeSliderState {
    let saver = RangeSliderState::Saver(on_value_change_finished.clone(), value_range.clone());
    remember_saveable_with(&saver, move || {
        RangeSliderState::new(active_range_start, active_range_end, steps, on_value_change_finished, value_range)
    })
}

// ---------------------------------------------------------------------------------------------------------------
// SliderRange

/// A range of floats packed in one long (Compose's internal value class `SliderRange`): what a range slider's
/// changes carry. Equal by the bits of its ends.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct SliderRange {
    packed_value: i64,
}

impl SliderRange {
    /// Compose's `SliderRange.Unspecified`.
    pub(crate) const Unspecified: SliderRange = SliderRange { packed_value: pack_floats(f32::NAN, f32::NAN) };

    /// Compose's `SliderRange(start, endInclusive)`: `start` ≤ `end_inclusive` (or both NaN, unspecified).
    pub(crate) fn new(start: f32, end_inclusive: f32) -> SliderRange {
        let is_unspecified = start.is_nan() && end_inclusive.is_nan();
        assert!(is_unspecified || start <= end_inclusive, "start({start}) must be <= endInclusive({end_inclusive})");
        SliderRange { packed_value: pack_floats(start, end_inclusive) }
    }

    /// Compose's `SliderRange(range: ClosedFloatingPointRange<Float>)` (unused in Compose's material3 too).
    #[allow(dead_code)]
    pub(crate) fn of(range: RangeInclusive<f32>) -> SliderRange {
        let (start, end_inclusive) = (*range.start(), *range.end());
        let is_unspecified = start.is_nan() && end_inclusive.is_nan();
        assert!(
            is_unspecified || start <= end_inclusive,
            "ClosedFloatingPointRange<Float>.start({start}) must be <= ClosedFloatingPoint.endInclusive({end_inclusive})"
        );
        SliderRange { packed_value: pack_floats(start, end_inclusive) }
    }

    pub(crate) fn start(&self) -> f32 {
        // Explicitly compare against packed values to avoid auto-boxing of Size.Unspecified
        assert!(self.packed_value != SliderRange::Unspecified.packed_value, "SliderRange is unspecified");
        unpack_float1(self.packed_value)
    }

    pub(crate) fn end_inclusive(&self) -> f32 {
        // Explicitly compare against packed values to avoid auto-boxing of Size.Unspecified
        assert!(self.packed_value != SliderRange::Unspecified.packed_value, "SliderRange is unspecified");
        unpack_float2(self.packed_value)
    }

    /// Compose's `SliderRange.isSpecified`.
    pub(crate) fn is_specified(&self) -> bool {
        self.packed_value != SliderRange::Unspecified.packed_value
    }
}

impl fmt::Debug for SliderRange {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_specified() {
            write!(f, "{}..{}", self.start().kt_to_string(), self.end_inclusive().kt_to_string())
        } else {
            write!(f, "FloatRange.Unspecified")
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// SliderPositions (deprecated)

/// Where a slider's active range and ticks are (Compose's deprecated SliderPositions, what the deprecated
/// [`PositionsTrack`](crate::SliderDefaults::PositionsTrack) draws): the active range and the ticks as fractions of
/// the track. A cheap handle: its clones are the same.
#[deprecated(note = "Not necessary with the introduction of Slider state")]
#[derive(Clone)]
pub struct SliderPositions {
    active_range: MutableState<RangeInclusive<f32>>,
    tick_fractions: MutableState<Vec<f32>>,
}

#[allow(deprecated)]
impl PartialEq for SliderPositions {
    /// Equal active ranges and ticks (Compose's `equals`: Kotlin's `ClosedFloatRange` equality — empty ranges are
    /// all equal, else their ends by `==` — and `FloatArray.contentEquals`, the ticks by `floatToIntBits`: all NaNs one).
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (self.active_range.peek(), other.active_range.peek());
        let ranges_equal = (a.is_empty() && b.is_empty()) || (a.start() == b.start() && a.end() == b.end());
        let (x, y) = (self.tick_fractions.peek(), other.tick_fractions.peek());
        ranges_equal
            && x.len() == y.len()
            && x.iter().zip(&y).all(|(p, q)| p.to_bits() == q.to_bits() || (p.is_nan() && q.is_nan()))
    }
}

/// Compose's deprecated `SliderPositions(initialActiveRange, initialTickFractions)`.
#[deprecated(note = "Not necessary with the introduction of Slider state")]
#[allow(deprecated)]
#[named(type)]
pub fn SliderPositions(
    #[default(0.0..=1.0)] initial_active_range: RangeInclusive<f32>,
    #[default(Vec::new())] initial_tick_fractions: Vec<f32>,
) -> SliderPositions {
    SliderPositions {
        active_range: mutable_state_of(initial_active_range),
        tick_fractions: mutable_state_of(initial_tick_fractions),
    }
}

#[allow(deprecated)]
impl SliderPositions {
    /// The active range, as fractions of the track (Compose's `activeRange`): read, it is observed.
    pub fn active_range(&self) -> RangeInclusive<f32> {
        self.active_range.get()
    }

    /// The ticks, as fractions of the track (Compose's `tickFractions`): read, it is observed.
    pub fn tick_fractions(&self) -> Vec<f32> {
        self.tick_fractions.get()
    }

    /// (Compose's `internal set` of `activeRange`; nothing in Compose's material3 sets it either.)
    #[allow(dead_code)]
    pub(crate) fn set_active_range(&self, active_range: RangeInclusive<f32>) {
        self.active_range.set(active_range);
    }

    /// (Compose's `internal set` of `tickFractions`; nothing in Compose's material3 sets it either.)
    #[allow(dead_code)]
    pub(crate) fn set_tick_fractions(&self, tick_fractions: Vec<f32>) {
        self.tick_fractions.set(tick_fractions);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The arithmetic

/// Kotlin's `coerceIn(min, max)` of a float: `min` if below it, `max` if above it (a range of `min` > `max` is an
/// error).
pub(crate) fn coerce_in(value: f32, min: f32, max: f32) -> f32 {
    assert!(
        min <= max || min.is_nan() || max.is_nan(),
        "Cannot coerce value to an empty range: maximum {max} is less than minimum {min}."
    );
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Kotlin's `coerceIn(range)` of a float in `lo..hi`: as [`coerce_in`], but an empty range (`lo` > `hi`, or a NaN
/// end) is an error.
#[track_caller]
pub(crate) fn coerce_in_range(value: f32, lo: f32, hi: f32) -> f32 {
    assert!(lo <= hi, "Cannot coerce value to an empty range: {}..{}.", lo.kt_to_string(), hi.kt_to_string());
    coerce_in(value, lo, hi)
}

/// The tick nearest `current` between `min_px` and `max_px`, or `current` without ticks (Compose's
/// `snapValueToTick`).
pub(crate) fn snap_value_to_tick(current: f32, tick_fractions: &[f32], min_px: f32, max_px: f32) -> f32 {
    // target is a closest anchor to the `current`, if exists
    let mut best: Option<(f32, f32)> = None;
    for &tick in tick_fractions {
        let distance = (lerp(min_px, max_px, tick) - current).abs();
        if best.is_none_or(|(_, d)| distance < d) {
            best = Some((tick, distance));
        }
    }
    best.map_or(current, |(tick, _)| lerp(min_px, max_px, tick))
}

/// The fractions of `steps` steps' ticks, the ends included; none without steps (Compose's
/// `stepsToTickFractions`).
pub(crate) fn steps_to_tick_fractions(steps: i32) -> Vec<f32> {
    // (Kotlin's `FloatArray(steps + 2)` throws for a negative size)
    assert!(steps + 2 >= 0, "NegativeArraySizeException: {}", steps + 2);
    if steps == 0 { Vec::new() } else { (0..steps + 2).map(|it| it as f32 / (steps + 1) as f32).collect() }
}

// Scale x1 from a1..b1 range to a2..b2 range
pub(crate) fn scale(a1: f32, b1: f32, x1: f32, a2: f32, b2: f32) -> f32 {
    lerp(a2, b2, calc_fraction(a1, b1, x1))
}

// Scale x.start, x.endInclusive from a1..b1 range to a2..b2 range
pub(crate) fn scale_range(is_start: bool, a1: f32, b1: f32, x: SliderRange, a2: f32, b2: f32) -> SliderRange {
    let start = scale(a1, b1, x.start(), a2, b2);
    let end = scale(a1, b1, x.end_inclusive(), a2, b2);
    if is_start { SliderRange::new(start.min(end), end) } else { SliderRange::new(start, end.max(start)) }
}

// Calculate the 0..1 fraction that `pos` value represents between `a` and `b`
pub(crate) fn calc_fraction(a: f32, b: f32, pos: f32) -> f32 {
    coerce_in(if b - a == 0.0 { 0.0 } else { (pos - a) / (b - a) }, 0.0, 1.0)
}
