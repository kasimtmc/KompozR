//! Material's sliders, as Compose's Slider.kt (material3 1.5): [`Slider`] (a value in a range, by a tap or a drag
//! along its track, the keyboard's arrows, accessibility's set-progress), its slots for the thumb and the track
//! ([`SlotSlider`]) and its state form ([`StateSlider`]); Expressive's [`VerticalSlider`]; [`RangeSlider`] (a range
//! by two thumbs, the nearer one taking a tap or a drag) in the same three forms; [`SliderDefaults`]' thumbs and
//! tracks, [`SliderColors`], the states ([`SliderState`], [`RangeSliderState`]).
//!
//! A slider keeps at least 48 dp of room to touch it; its thumb is a 4 by 44 dp bar, so the track starts 2 dp in;
//! the stretches of the track keep 6 dp from the thumb.

mod defaults;
mod state;

#[allow(deprecated)]
pub use defaults::{DrawStopIndicator, DrawTick, SliderColors, SliderDefaults};
#[allow(deprecated)]
pub use state::SliderPositions;
pub use state::{RangeSliderState, SliderState, remember_range_slider_state, remember_slider_state};

use crate::interactive_component_size::InteractiveComponentSizeModifiers;
use crate::internal::accessibility_util::{IncreaseHorizontalSemanticsBounds, IncreaseVerticalSemanticsBounds};
use crate::internal::drag_gesture_detector_copy::{await_horizontal_pointer_slop_or_cancellation, pointer_slop};
use crate::internal::strings::{Strings, get_string};
use kompoz_coroutines::{CoroutineScope, coroutine_scope};
use kompoz_foundation::gestures::{
    AwaitPointerEventScopeGestures, DraggableModifiers, OnDragStopped, Orientation, PointerInputScopeGestures,
};
use kompoz_foundation::interaction::{DragInteraction, Interaction, MutableInteractionSource, PressInteraction};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Dp, DpSize, LayoutDirection, Offset, lerp};
use kompoz_runtime::kt::KtToString;
use kompoz_runtime::{kompozable, named_args, remember, remember_keyed};
use kompoz_ui::input::{
    AwaitPointerEventScope, Key, KeyEvent, KeyEventType, PointerId, PointerInputChange, PointerInputScope, PointerType,
};
use kompoz_ui::layout::{AlignmentLine, Layout, Measurable, VerticalAlignmentLine, measure_policy};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::semantics::SemanticsConfiguration;
use kompoz_ui::{Modifier, Placeable};
use state::{SliderRange, coerce_in, coerce_in_range};
use std::cell::Cell;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// Slider

/// A slider (Compose's `Slider(value, onValueChange, …)`): `value` in `value_range` — a tap or a drag along the
/// track, the arrow keys (1% a press without steps), Home/End, Page Up/Down, accessibility's set-progress —
/// `on_value_change` given each new value (snapped to the nearest of `steps` evenly spread values between the
/// range's ends; none: any value), `on_value_change_finished` called when a change ends. Not `enabled`: faded,
/// taking no input. Its colors are `colors`'; its interactions are emitted to `interaction_source` (one of its
/// own where none). A vertical drag over it isn't its: a scrolling column around it takes it.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn Slider(
    value: f32,
    on_value_change: impl Fn(f32) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
    #[default(0)] steps: i32,
    #[default(None)] on_value_change_finished: Option<Rc<dyn Fn()>>,
    #[default(SliderDefaults.colors())] colors: SliderColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let thumb = default_thumb(interaction_source.clone(), colors, enabled);
    let track = default_track(colors, enabled);
    SlotSlider(
        value = value,
        on_value_change = on_value_change,
        modifier = modifier,
        enabled = enabled,
        on_value_change_finished = on_value_change_finished,
        colors = colors,
        interaction_source = interaction_source,
        steps = steps,
        thumb = thumb,
        track = track,
        value_range = value_range,
    );
}

/// A slider with a thumb and a track of its own (Compose's `Slider(value, onValueChange, …, thumb, track,
/// valueRange)`): `thumb` and `track` are composed with the slider's state (none: [`SliderDefaults::Thumb`] and
/// [`SliderDefaults::Track`] in `colors`), the track measured as wide as the slider less the thumb, from half a
/// thumb in, the thumb's middle at the value along it. The rest as [`Slider`]'s.
///
/// Kotlin: `Slider(value: Float, onValueChange: (Float) -> Unit, thumb: @Composable (SliderState) -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SlotSlider(
    value: f32,
    on_value_change: impl Fn(f32) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] on_value_change_finished: Option<Rc<dyn Fn()>>,
    #[default(SliderDefaults.colors())] colors: SliderColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(0)] steps: i32,
    #[default(None)] thumb: Option<Rc<dyn Fn(SliderState)>>,
    #[default(None)] track: Option<Rc<dyn Fn(SliderState)>>,
    #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let thumb = thumb.unwrap_or_else(|| default_thumb(interaction_source.clone(), colors, enabled));
    let track = track.unwrap_or_else(|| default_track(colors, enabled));
    let (initial, finished, range) = (value, on_value_change_finished.clone(), value_range.clone());
    let state = remember_keyed((steps, value_range), move || SliderState::new(initial, steps, finished, range));
    state.set_on_value_change_finished(on_value_change_finished);
    state.set_on_value_change(Some(Rc::new(on_value_change) as Rc<dyn Fn(f32)>));
    state.set_value(value);

    StateSlider(
        state = state,
        modifier = modifier,
        enabled = enabled,
        interaction_source = interaction_source,
        thumb = thumb,
        track = track,
    );
}

/// A slider of `state` (Compose's `Slider(state, …)`): the value, its range, steps and callbacks are the state's.
/// The rest as [`SlotSlider`]'s.
///
/// Kotlin: `Slider(state: SliderState)`
#[kompozable]
pub fn StateSlider(
    state: SliderState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(SliderDefaults.colors())] colors: SliderColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(None)] thumb: Option<Rc<dyn Fn(SliderState)>>,
    #[default(None)] track: Option<Rc<dyn Fn(SliderState)>>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let thumb = thumb.unwrap_or_else(|| default_thumb(interaction_source.clone(), colors, enabled));
    let track = track.unwrap_or_else(|| default_track(colors, enabled));
    assert!(state.steps() >= 0, "steps should be >= 0");

    SliderImpl(state, modifier, enabled, interaction_source, thumb, track);
}

/// A slider standing up (Compose's Expressive `VerticalSlider(state, …)`): the value grows from the top down,
/// from the bottom up with `reverse_direction`; a vertical drag moves it. Its default thumb lies across it, its
/// default track's corners are half its width. The rest as [`StateSlider`]'s.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn VerticalSlider(
    state: SliderState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(false)] reverse_direction: bool,
    #[default(SliderDefaults.colors())] colors: SliderColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(None)] thumb: Option<Rc<dyn Fn(SliderState)>>,
    #[default(None)] track: Option<Rc<dyn Fn(SliderState)>>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let thumb_source = interaction_source.clone();
    let thumb = thumb.unwrap_or_else(|| {
        Rc::new(move |slider_state: SliderState| {
            named_args!(SliderDefaults.StateThumb(
                thumb_source.clone(),
                slider_state,
                colors = colors,
                enabled = enabled,
                thumb_size = VerticalThumbSize
            ))
        })
    });
    let track = track.unwrap_or_else(|| {
        Rc::new(move |slider_state: SliderState| {
            named_args!(SliderDefaults.CornerSizeTrack(
                slider_state,
                Dp::Unspecified,
                colors = colors,
                enabled = enabled
            ))
        })
    });
    assert!(state.steps() >= 0, "steps should be >= 0");

    state.set_orientation(Orientation::Vertical);
    state.set_reverse_vertical_direction(reverse_direction);

    SliderImpl(state, modifier, enabled, interaction_source, thumb, track);
}

/// Compose's default `thumb`: [`SliderDefaults::Thumb`] of the source, in `colors`.
fn default_thumb(
    interaction_source: MutableInteractionSource,
    colors: SliderColors,
    enabled: bool,
) -> Rc<dyn Fn(SliderState)> {
    Rc::new(move |_| {
        named_args!(SliderDefaults.Thumb(interaction_source.clone(), colors = colors, enabled = enabled));
    })
}

/// Compose's default `track`: [`SliderDefaults::Track`] in `colors`.
fn default_track(colors: SliderColors, enabled: bool) -> Rc<dyn Fn(SliderState)> {
    Rc::new(move |slider_state: SliderState| {
        named_args!(SliderDefaults.Track(colors = colors, enabled = enabled, slider_state = slider_state));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// RangeSlider

/// A range slider (Compose's `RangeSlider(value, onValueChange, …)`): a range in `value_range` by two thumbs — a
/// tap or a drag moves the nearer one (the drag's way picks one of two together), each thumb its arrow keys and
/// its accessibility node —, `on_value_change` given each new range (its ends snapped to the nearest of `steps`
/// evenly spread values), `on_value_change_finished` called when a change ends. Not `enabled`: faded, taking no
/// input. A vertical drag over it isn't its.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn RangeSlider(
    value: RangeInclusive<f32>,
    on_value_change: impl Fn(RangeInclusive<f32>) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
    #[default(0)] steps: i32,
    #[default(None)] on_value_change_finished: Option<Rc<dyn Fn()>>,
    #[default(SliderDefaults.colors())] colors: SliderColors,
) {
    let start_interaction_source = remember(MutableInteractionSource::new);
    let end_interaction_source = remember(MutableInteractionSource::new);

    let start_thumb = default_range_thumb(start_interaction_source.clone(), colors, enabled);
    let end_thumb = default_range_thumb(end_interaction_source.clone(), colors, enabled);
    let track = default_range_track(colors, enabled);
    SlotRangeSlider(
        value = value,
        on_value_change = on_value_change,
        modifier = modifier,
        enabled = enabled,
        value_range = value_range,
        steps = steps,
        on_value_change_finished = on_value_change_finished,
        start_interaction_source = start_interaction_source,
        end_interaction_source = end_interaction_source,
        start_thumb = start_thumb,
        end_thumb = end_thumb,
        track = track,
    );
}

/// A range slider with thumbs and a track of its own (Compose's `RangeSlider(value, onValueChange, …,
/// startInteractionSource, endInteractionSource, startThumb, endThumb, track, steps)`): each composed with the
/// slider's state (none: [`SliderDefaults::Thumb`]s of their sources, [`SliderDefaults::RangeTrack`], in `colors`);
/// each thumb's interactions go to its own source. The rest as [`RangeSlider`]'s.
///
/// Kotlin: `RangeSlider(value: ClosedFloatingPointRange<Float>, onValueChange: (ClosedFloatingPointRange<Float>) -> Unit, startThumb: @Composable (RangeSliderState) -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SlotRangeSlider(
    value: RangeInclusive<f32>,
    on_value_change: impl Fn(RangeInclusive<f32>) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(0.0..=1.0)] value_range: RangeInclusive<f32>,
    #[default(None)] on_value_change_finished: Option<Rc<dyn Fn()>>,
    #[default(SliderDefaults.colors())] colors: SliderColors,
    #[default(None)] start_interaction_source: Option<MutableInteractionSource>,
    #[default(None)] end_interaction_source: Option<MutableInteractionSource>,
    #[default(None)] start_thumb: Option<Rc<dyn Fn(RangeSliderState)>>,
    #[default(None)] end_thumb: Option<Rc<dyn Fn(RangeSliderState)>>,
    #[default(None)] track: Option<Rc<dyn Fn(RangeSliderState)>>,
    #[default(0)] steps: i32,
) {
    let start_interaction_source = start_interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let end_interaction_source = end_interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let start_thumb =
        start_thumb.unwrap_or_else(|| default_range_thumb(start_interaction_source.clone(), colors, enabled));
    let end_thumb = end_thumb.unwrap_or_else(|| default_range_thumb(end_interaction_source.clone(), colors, enabled));
    let track = track.unwrap_or_else(|| default_range_track(colors, enabled));
    let (start, end, finished, range) =
        (*value.start(), *value.end(), on_value_change_finished.clone(), value_range.clone());
    let state = remember_keyed((steps, value_range), move || RangeSliderState::new(start, end, steps, finished, range));

    state.set_on_value_change_finished(on_value_change_finished);
    let on_value_change = Rc::new(on_value_change);
    let on_range_change: Rc<dyn Fn(SliderRange)> =
        Rc::new(move |it: SliderRange| on_value_change(it.start()..=it.end_inclusive()));
    state.set_on_value_change(Some(on_range_change));
    state.set_active_range_start(*value.start());
    state.set_active_range_end(*value.end());

    StateRangeSlider(
        modifier = modifier,
        state = state,
        enabled = enabled,
        start_interaction_source = start_interaction_source,
        end_interaction_source = end_interaction_source,
        start_thumb = start_thumb,
        end_thumb = end_thumb,
        track = track,
    );
}

/// A range slider of `state` (Compose's `RangeSlider(state, …)`): the range, its steps and callbacks are the
/// state's. The rest as [`SlotRangeSlider`]'s.
///
/// Kotlin: `RangeSlider(state: RangeSliderState)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StateRangeSlider(
    state: RangeSliderState,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(SliderDefaults.colors())] colors: SliderColors,
    #[default(None)] start_interaction_source: Option<MutableInteractionSource>,
    #[default(None)] end_interaction_source: Option<MutableInteractionSource>,
    #[default(None)] start_thumb: Option<Rc<dyn Fn(RangeSliderState)>>,
    #[default(None)] end_thumb: Option<Rc<dyn Fn(RangeSliderState)>>,
    #[default(None)] track: Option<Rc<dyn Fn(RangeSliderState)>>,
) {
    let start_interaction_source = start_interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let end_interaction_source = end_interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    let start_thumb =
        start_thumb.unwrap_or_else(|| default_range_thumb(start_interaction_source.clone(), colors, enabled));
    let end_thumb = end_thumb.unwrap_or_else(|| default_range_thumb(end_interaction_source.clone(), colors, enabled));
    let track = track.unwrap_or_else(|| default_range_track(colors, enabled));
    assert!(state.steps() >= 0, "steps should be >= 0");

    RangeSliderImpl(
        modifier,
        state,
        enabled,
        start_interaction_source,
        end_interaction_source,
        start_thumb,
        end_thumb,
        track,
    );
}

/// Compose's default `startThumb`/`endThumb`: [`SliderDefaults::Thumb`] of the thumb's source, in `colors`.
fn default_range_thumb(
    interaction_source: MutableInteractionSource,
    colors: SliderColors,
    enabled: bool,
) -> Rc<dyn Fn(RangeSliderState)> {
    Rc::new(move |_| {
        named_args!(SliderDefaults.Thumb(interaction_source.clone(), colors = colors, enabled = enabled));
    })
}

/// Compose's default range `track`: [`SliderDefaults::RangeTrack`] in `colors`.
fn default_range_track(colors: SliderColors, enabled: bool) -> Rc<dyn Fn(RangeSliderState)> {
    Rc::new(move |range_slider_state: RangeSliderState| {
        named_args!(SliderDefaults.RangeTrack(range_slider_state, colors = colors, enabled = enabled));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The layouts

/// The parts of a slider's layout (Compose's private enum `SliderComponents`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SliderComponents {
    Thumb,
    Track,
}

/// The parts of a range slider's layout (Compose's private enum `RangeSliderComponents`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RangeSliderComponents {
    EndThumb,
    StartThumb,
    Track,
}

/// The measurable of the part `id` (Compose's `measurables.fastFirst { it.layoutId == id }`).
fn part<'a, T: std::any::Any + PartialEq>(measurables: &[&'a dyn Measurable], id: T) -> &'a dyn Measurable {
    measurables
        .iter()
        .copied()
        .find(|m| m.layout_id().is_some_and(|i| i.is(&id)))
        .expect("Collection contains no element matching the predicate.")
}

/// The corner size the track tells (Compose's `trackPlaceable[CornerSizeAlignmentLine]`, 0 where it tells none).
fn track_corner_size_of(track: &Placeable) -> i32 {
    let it = track.get(CornerSizeAlignmentLine);
    if it != AlignmentLine::Unspecified { it } else { 0 }
}

/// Kotlin's `Float.roundToInt()` (Java's `Math.round`: halves up; a NaN throws).
#[track_caller]
fn round_to_int(x: f32) -> i32 {
    kompoz_runtime::kt::KtFloat::round_to_int(x)
}

/// A slider's layout (Compose's private `SliderImpl`): the thumb and the track, its gestures, keys and semantics.
#[kompozable(plain(offset))]
fn SliderImpl(
    state: SliderState,
    modifier: Modifier,
    enabled: bool,
    interaction_source: MutableInteractionSource,
    thumb: Rc<dyn Fn(SliderState)>,
    track: Rc<dyn Fn(SliderState)>,
) {
    state.set_is_rtl(LocalLayoutDirection.current() == LayoutDirection::Rtl);
    let vertical = state.orientation() == Orientation::Vertical;
    let reverse_direction = (!vertical && state.is_rtl()) || (vertical && state.reverse_vertical_direction());
    let press = slider_tap_modifier(&state, &interaction_source, enabled);
    let end = state.clone();
    let drag = named_args!(Modifier.draggable(
        state.clone(),
        state.orientation(),
        reverse_direction = reverse_direction,
        enabled = enabled,
        interaction_source = Some(interaction_source.clone()),
        on_drag_stopped = Some(Rc::new(move |_, _| {
            end.gesture_end_action();
            Box::pin(async {}) as kompoz_foundation::gestures::LocalFuture<'static, ()>
        }) as OnDragStopped),
        start_drag_immediately = state.is_dragging()
    ));
    let thumb_modifier = if vertical {
        Modifier.layout_id(SliderComponents::Thumb).wrap_content_height()
    } else {
        Modifier.layout_id(SliderComponents::Thumb).wrap_content_width()
    };

    let key_state = state.clone();
    let on_key_value: Rc<dyn Fn(f32)> = Rc::new(move |updated_value| match key_state.on_value_change() {
        Some(on_value_change) => on_value_change(updated_value),
        None => key_state.set_value(updated_value),
    });
    let (thumb_state, track_state, measure_state) = (state.clone(), state.clone(), state.clone());
    Layout(
        content = move || {
            let (size_state, slot_state) = (thumb_state.clone(), thumb_state.clone());
            let thumb = thumb.clone();
            Box(
                modifier =
                    thumb_modifier.clone().on_size_changed(move |it| size_state.set_thumb_size(it.width, it.height)),
                content = move || thumb(slot_state.clone()),
            );
            let (track, track_state) = (track.clone(), track_state.clone());
            Box(modifier = Modifier.layout_id(SliderComponents::Track), content = move || track(track_state.clone()));
        },
        modifier = modifier
            .minimum_interactive_component_size()
            .required_size_in_with(
                if vertical { TrackHeight } else { ThumbWidth },
                if vertical { ThumbWidth } else { TrackHeight },
                Dp::Unspecified,
                Dp::Unspecified,
            )
            .then(slider_semantics(&state, enabled))
            .focusable_with(enabled, Some(interaction_source.clone()))
            .then(slide_on_key_events(
                enabled,
                state.steps(),
                state.value_range(),
                state.value(),
                reverse_direction,
                on_key_value,
                state.on_value_change_finished(),
                state.is_rtl(),
                vertical,
            ))
            .then(press)
            .then(drag),
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let state = &measure_state;
            let thumb_placeable = part(measurables, SliderComponents::Thumb).measure(constraints);

            let track_measurable = part(measurables, SliderComponents::Track);
            let track_placeable = if state.orientation() == Orientation::Vertical {
                let c = constraints.offset(0, -thumb_placeable.height());
                track_measurable.measure(named_args!(c.copy(min_width = 0)))
            } else {
                let c = constraints.offset(-thumb_placeable.width(), 0);
                track_measurable.measure(named_args!(c.copy(min_height = 0)))
            };

            let value_as_fraction = state.coerced_value_as_fraction();
            let ticks = state.tick_fractions();
            let is_on_first_or_last_step =
                Some(value_as_fraction) == ticks.first().copied() || Some(value_as_fraction) == ticks.last().copied();
            let track_corner_size = track_corner_size_of(&track_placeable);

            let (slider_width, slider_height, track_offset_x, track_offset_y, thumb_offset_x, thumb_offset_y);
            if state.orientation() == Orientation::Vertical {
                slider_width = track_placeable.width().max(thumb_placeable.width());
                slider_height = thumb_placeable.height() + track_placeable.height();
                track_offset_x = (slider_width - track_placeable.width()) / 2;
                track_offset_y = thumb_placeable.height() / 2;
                thumb_offset_x = (slider_width - thumb_placeable.width()) / 2;
                let mut offset_y = if state.steps() > 0 && !is_on_first_or_last_step {
                    round_to_int((track_placeable.height() - track_corner_size * 2) as f32 * value_as_fraction)
                        + track_corner_size
                } else {
                    round_to_int(track_placeable.height() as f32 * value_as_fraction)
                };
                if state.reverse_vertical_direction() {
                    offset_y = track_placeable.height() - offset_y;
                }
                thumb_offset_y = offset_y;
            } else {
                slider_width = thumb_placeable.width() + track_placeable.width();
                slider_height = track_placeable.height().max(thumb_placeable.height());
                track_offset_x = thumb_placeable.width() / 2;
                track_offset_y = (slider_height - track_placeable.height()) / 2;
                thumb_offset_x = if state.steps() > 0 && !is_on_first_or_last_step {
                    round_to_int((track_placeable.width() - track_corner_size * 2) as f32 * value_as_fraction)
                        + track_corner_size
                } else {
                    round_to_int(track_placeable.width() as f32 * value_as_fraction)
                };
                thumb_offset_y = (slider_height - thumb_placeable.height()) / 2;
            }

            state.update_dimensions(slider_width, slider_height);

            scope.layout(slider_width, slider_height, move |p| {
                p.place_relative(&track_placeable, track_offset_x, track_offset_y);
                p.place_relative(&thumb_placeable, thumb_offset_x, thumb_offset_y);
            })
        }),
    );
}

/// A range slider's layout (Compose's private `RangeSliderImpl`): the two thumbs (each its semantics, keys and
/// focus) and the track, the press and drag of the nearer thumb.
#[kompozable(plain(offset))]
#[allow(clippy::too_many_arguments)]
fn RangeSliderImpl(
    modifier: Modifier,
    state: RangeSliderState,
    enabled: bool,
    start_interaction_source: MutableInteractionSource,
    end_interaction_source: MutableInteractionSource,
    start_thumb: Rc<dyn Fn(RangeSliderState)>,
    end_thumb: Rc<dyn Fn(RangeSliderState)>,
    track: Rc<dyn Fn(RangeSliderState)>,
) {
    state.set_is_rtl(LocalLayoutDirection.current() == LayoutDirection::Rtl);

    let press_drag =
        range_slider_press_drag_modifier(&state, &start_interaction_source, &end_interaction_source, enabled);

    let start_content_description = get_string(Strings::SliderRangeStart);
    let end_content_description = get_string(Strings::SliderRangeEnd);

    let start_key_state = state.clone();
    let on_start_key: Rc<dyn Fn(SliderRange)> = Rc::new(move |slider_range| match start_key_state.on_value_change() {
        Some(on_value_change) => on_value_change(slider_range),
        None => start_key_state.set_active_range_start(slider_range.start()),
    });
    let end_key_state = state.clone();
    let on_end_key: Rc<dyn Fn(SliderRange)> = Rc::new(move |slider_range| match end_key_state.on_value_change() {
        Some(on_value_change) => on_value_change(slider_range),
        None => end_key_state.set_active_range_end(slider_range.end_inclusive()),
    });
    let start_size_state = state.clone();
    let start_modifier = Modifier
        .layout_id(RangeSliderComponents::StartThumb)
        .wrap_content_width()
        .on_size_changed(move |it| start_size_state.set_start_thumb_size(it.width as f32, it.height as f32))
        .then(range_slider_start_thumb_semantics(&state, enabled))
        .semantics_with(true, move |c: &mut SemanticsConfiguration| {
            c.set_content_description(start_content_description.clone())
        })
        .then(range_slider_on_key_events(
            enabled,
            state.steps(),
            state.value_range(),
            state.active_range_start(),
            state.active_range_end(),
            true,
            state.is_rtl(),
            on_start_key,
            state.on_value_change_finished(),
        ))
        .focusable_with(enabled, Some(start_interaction_source.clone()));
    let end_size_state = state.clone();
    let end_modifier = Modifier
        .layout_id(RangeSliderComponents::EndThumb)
        .wrap_content_width()
        .on_size_changed(move |it| end_size_state.set_end_thumb_size(it.width as f32, it.height as f32))
        .then(range_slider_end_thumb_semantics(&state, enabled))
        .semantics_with(true, move |c: &mut SemanticsConfiguration| {
            c.set_content_description(end_content_description.clone())
        })
        .then(range_slider_on_key_events(
            enabled,
            state.steps(),
            state.value_range(),
            state.active_range_start(),
            state.active_range_end(),
            false,
            state.is_rtl(),
            on_end_key,
            state.on_value_change_finished(),
        ))
        .focusable_with(enabled, Some(end_interaction_source.clone()));

    let (slot_state, measure_state) = (state.clone(), state.clone());
    Layout(
        content = move || {
            let (start_thumb, end_thumb, track) = (start_thumb.clone(), end_thumb.clone(), track.clone());
            let (s1, s2, s3) = (slot_state.clone(), slot_state.clone(), slot_state.clone());
            Box(modifier = start_modifier.clone(), content = move || start_thumb(s1.clone()));
            Box(modifier = end_modifier.clone(), content = move || end_thumb(s2.clone()));
            Box(modifier = Modifier.layout_id(RangeSliderComponents::Track), content = move || track(s3.clone()));
        },
        modifier = modifier
            .minimum_interactive_component_size()
            .required_size_in_with(ThumbWidth, TrackHeight, Dp::Unspecified, Dp::Unspecified)
            .then(press_drag),
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            let state = &measure_state;
            let start_thumb_placeable = part(measurables, RangeSliderComponents::StartThumb).measure(constraints);

            let end_thumb_placeable = part(measurables, RangeSliderComponents::EndThumb).measure(constraints);

            let c = constraints.offset(-(start_thumb_placeable.width() + end_thumb_placeable.width()) / 2, 0);
            let track_placeable =
                part(measurables, RangeSliderComponents::Track).measure(named_args!(c.copy(min_height = 0)));

            let slider_width =
                track_placeable.width() + (start_thumb_placeable.width() + end_thumb_placeable.width()) / 2;
            let slider_height =
                track_placeable.height().max(start_thumb_placeable.height()).max(end_thumb_placeable.height());

            state.set_total_width(slider_width);

            state.update_min_max_px();

            let ticks = state.tick_fractions();
            let (first, last) = (ticks.first().copied(), ticks.last().copied());
            let start_value_as_fraction = state.coerced_active_range_start_as_fraction();
            let is_start_on_first_or_last_step =
                Some(start_value_as_fraction) == first || Some(start_value_as_fraction) == last;
            let end_value_as_fraction = state.coerced_active_range_end_as_fraction();
            let is_end_on_first_or_last_step =
                Some(end_value_as_fraction) == first || Some(end_value_as_fraction) == last;
            let track_offset_x = start_thumb_placeable.width() / 2;
            let track_corner_size = track_corner_size_of(&track_placeable);

            let start_thumb_offset_x = if state.steps() > 0 && !is_start_on_first_or_last_step {
                round_to_int((track_placeable.width() - track_corner_size * 2) as f32 * start_value_as_fraction)
                    + track_corner_size
            } else {
                round_to_int(track_placeable.width() as f32 * start_value_as_fraction)
            };
            // When start thumb and end thumb have different widths,
            // we need to add a correction for the centering of the slider.
            let end_correction = (start_thumb_placeable.width() - end_thumb_placeable.width()) / 2;
            let end_thumb_offset_x = if state.steps() > 0 && !is_end_on_first_or_last_step {
                round_to_int(
                    (track_placeable.width() - track_corner_size * 2) as f32 * end_value_as_fraction
                        + end_correction as f32,
                ) + track_corner_size
            } else {
                round_to_int(track_placeable.width() as f32 * end_value_as_fraction + end_correction as f32)
            };
            let track_offset_y = (slider_height - track_placeable.height()) / 2;
            let start_thumb_offset_y = (slider_height - start_thumb_placeable.height()) / 2;
            let end_thumb_offset_y = (slider_height - end_thumb_placeable.height()) / 2;

            scope.layout(slider_width, slider_height, move |p| {
                p.place_relative(&track_placeable, track_offset_x, track_offset_y);
                p.place_relative(&start_thumb_placeable, start_thumb_offset_x, start_thumb_offset_y);
                p.place_relative(&end_thumb_placeable, end_thumb_offset_x, end_thumb_offset_y);
            })
        }),
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Keys

/// The page of a page key (Compose's `(actualSteps / 10).coerceIn(1, 10)`).
fn page_of(actual_steps: i32) -> i32 {
    (actual_steps / 10).clamp(1, 10)
}

/// A slider's keys (Compose's private `Modifier.slideOnKeyEvents`): the arrows along it (the other way when
/// reversed) by a step (1% of the range without steps), Page Up/Down by a tenth of the steps (1 to 10), Home/End
/// to the ends; their release ends the change.
#[allow(clippy::too_many_arguments)]
fn slide_on_key_events(
    enabled: bool,
    steps: i32,
    value_range: RangeInclusive<f32>,
    value: f32,
    reverse_direction: bool,
    on_value_change_state: Rc<dyn Fn(f32)>,
    on_value_change_finished_state: Option<Rc<dyn Fn()>>,
    _is_rtl: bool,
    is_vertical: bool,
) -> Modifier {
    assert!(steps >= 0, "steps should be >= 0");
    let (start, end) = (*value_range.start(), *value_range.end());
    Modifier.on_key_event(move |it: &KeyEvent| {
        if !enabled {
            return false;
        }
        match it.r#type {
            KeyEventType::KeyDown => {
                let range_length = (end - start).abs();
                // When steps == 0, it means that a user is not limited by a step length (delta)
                // when using touch or mouse. But it is not possible to adjust the value
                // continuously when using keyboard buttons - the delta has to be discrete.
                // In this case, 1% of the valueRange seems to make sense.
                let actual_steps = if steps > 0 { steps + 1 } else { 100 };
                let delta = range_length / actual_steps as f32;
                let sign = if reverse_direction { -1.0 } else { 1.0 };

                if it.key == Key::MoveHome {
                    on_value_change_state(start);
                    return true;
                } else if it.key == Key::MoveEnd {
                    on_value_change_state(end);
                    return true;
                }
                let page = page_of(actual_steps) as f32;
                if is_vertical {
                    match it.key {
                        Key::DirectionUp => on_value_change_state(coerce_in_range(value - sign * delta, start, end)),
                        Key::DirectionDown => on_value_change_state(coerce_in_range(value + sign * delta, start, end)),
                        Key::PageUp => on_value_change_state(coerce_in_range(value - page * sign * delta, start, end)),
                        Key::PageDown => {
                            on_value_change_state(coerce_in_range(value + page * sign * delta, start, end))
                        }
                        _ => return false,
                    }
                } else {
                    match it.key {
                        Key::DirectionRight => on_value_change_state(coerce_in_range(value + sign * delta, start, end)),
                        Key::DirectionLeft => on_value_change_state(coerce_in_range(value - sign * delta, start, end)),
                        Key::PageUp => on_value_change_state(coerce_in_range(value + page * delta, start, end)),
                        Key::PageDown => on_value_change_state(coerce_in_range(value - page * delta, start, end)),
                        _ => return false,
                    }
                }
                true
            }
            KeyEventType::KeyUp => {
                let along = if is_vertical {
                    [Key::DirectionUp, Key::DirectionDown]
                } else {
                    [Key::DirectionRight, Key::DirectionLeft]
                };
                if along.contains(&it.key)
                    || [Key::MoveHome, Key::MoveEnd, Key::PageUp, Key::PageDown].contains(&it.key)
                {
                    if let Some(f) = &on_value_change_finished_state {
                        f();
                    }
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    })
}

/// A range slider thumb's keys (Compose's private `Modifier.rangeSliderOnKeyEvents`): as a slider's, the start
/// thumb's kept before the end, the end thumb's after the start.
#[allow(clippy::too_many_arguments)]
fn range_slider_on_key_events(
    enabled: bool,
    steps: i32,
    value_range: RangeInclusive<f32>,
    value_start: f32,
    value_end: f32,
    is_start_thumb: bool,
    reverse_direction: bool,
    on_value_change_state: Rc<dyn Fn(SliderRange)>,
    on_value_change_finished_state: Option<Rc<dyn Fn()>>,
) -> Modifier {
    assert!(steps >= 0, "steps should be >= 0");
    let (start, end) = (*value_range.start(), *value_range.end());
    Modifier.on_key_event(move |it: &KeyEvent| {
        if !enabled {
            return false;
        }
        match it.r#type {
            KeyEventType::KeyDown => {
                let range_length = (end - start).abs();
                // When steps == 0, it means that a user is not limited by a step length (delta)
                // when using touch or mouse. But it is not possible to adjust the value
                // continuously when using keyboard buttons - the delta has to be discrete.
                // In this case, 1% of the valueRange seems to make sense.
                let actual_steps = if steps > 0 { steps + 1 } else { 100 };
                let delta = range_length / actual_steps as f32;
                let sign = if reverse_direction { -1.0 } else { 1.0 };
                let page = page_of(actual_steps) as f32;

                if is_start_thumb {
                    let (lo, hi) = (start, value_end);
                    let range = match it.key {
                        Key::DirectionRight => {
                            SliderRange::new(coerce_in_range(value_start + sign * delta, lo, hi), value_end)
                        }
                        Key::DirectionLeft => {
                            SliderRange::new(coerce_in_range(value_start - sign * delta, lo, hi), value_end)
                        }
                        Key::PageUp => SliderRange::new(coerce_in_range(value_start + page * delta, lo, hi), value_end),
                        Key::PageDown => {
                            SliderRange::new(coerce_in_range(value_start - page * delta, lo, hi), value_end)
                        }
                        Key::MoveHome => SliderRange::new(start, value_end),
                        Key::MoveEnd => SliderRange::new(value_end, value_end),
                        _ => return false,
                    };
                    on_value_change_state(range);
                } else {
                    let (lo, hi) = (value_start, end);
                    let range = match it.key {
                        Key::DirectionRight => {
                            SliderRange::new(value_start, coerce_in_range(value_end + sign * delta, lo, hi))
                        }
                        Key::DirectionLeft => {
                            SliderRange::new(value_start, coerce_in_range(value_end - sign * delta, lo, hi))
                        }
                        Key::PageUp => SliderRange::new(value_start, coerce_in_range(value_end + page * delta, lo, hi)),
                        Key::PageDown => {
                            SliderRange::new(value_start, coerce_in_range(value_end - page * delta, lo, hi))
                        }
                        Key::MoveHome => SliderRange::new(value_start, value_start),
                        Key::MoveEnd => SliderRange::new(value_start, end),
                        _ => return false,
                    };
                    on_value_change_state(range);
                }
                true
            }
            KeyEventType::KeyUp => {
                let keys =
                    [Key::DirectionRight, Key::DirectionLeft, Key::MoveHome, Key::MoveEnd, Key::PageUp, Key::PageDown];
                if keys.contains(&it.key) {
                    if let Some(f) = &on_value_change_finished_state {
                        f();
                    }
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Semantics

/// Kotlin's `"${(this * 100).roundToInt() / 100f}"` (Compose's private `Float.formatForSemantics`).
fn format_for_semantics(value: f32) -> String {
    (round_to_int(value * 100.0) as f32 / 100.0).kt_to_string()
}

/// `target_value` within `lo..=hi`, snapped to the nearest of `steps` steps' values there (Compose's set-progress
/// resolution).
fn resolve_progress(target_value: f32, lo: f32, hi: f32, steps: i32) -> f32 {
    let mut new_value = coerce_in(target_value, lo, hi);
    let original_val = new_value;
    if steps > 0 {
        let mut distance: f32 = new_value;
        for i in 0..=steps + 1 {
            let step_value = lerp(lo, hi, i as f32 / (steps + 1) as f32);
            if (step_value - original_val).abs() <= distance {
                distance = (step_value - original_val).abs();
                new_value = step_value;
            }
        }
    }
    new_value
}

/// What a slider tells accessibility (Compose's private `Modifier.sliderSemantics`): its value as its state
/// description, a set-progress action (snapping to the steps; false where it changes nothing), disabled where it
/// is, 10 dp more each way across for a screen reader's box, its progress in its range and steps.
fn slider_semantics(state: &SliderState, enabled: bool) -> Modifier {
    let s = state.clone();
    Modifier
        .semantics_with(false, move |c: &mut SemanticsConfiguration| {
            if !enabled {
                c.disabled();
            }
            c.set_state_description(format_for_semantics(s.value()));
            let state = s.clone();
            c.set_progress(move |target_value: f32| {
                let range = state.value_range();
                let resolved_value = resolve_progress(target_value, *range.start(), *range.end(), state.steps());

                // This is to keep it consistent with AbsSeekbar.java: return false if no
                // change from current.
                if resolved_value == state.value() {
                    false
                } else {
                    if resolved_value != state.value() {
                        match state.on_value_change() {
                            Some(on_value_change) => on_value_change(resolved_value),
                            None => state.set_value(resolved_value),
                        }
                    }
                    if let Some(f) = state.on_value_change_finished() {
                        f();
                    }
                    true
                }
            });
        })
        .then(if state.orientation() == Orientation::Vertical {
            IncreaseVerticalSemanticsBounds()
        } else {
            IncreaseHorizontalSemanticsBounds()
        })
        .progress_semantics_with(
            state.value(),
            *state.value_range().start()..=*state.value_range().end(),
            state.steps(),
        )
}

/// What a range slider's start thumb tells accessibility (Compose's private
/// `Modifier.rangeSliderStartThumbSemantics`): as a slider's, in the range up to the end thumb.
fn range_slider_start_thumb_semantics(state: &RangeSliderState, enabled: bool) -> Modifier {
    let value_range = *state.value_range().start()..=state.active_range_end();
    let (s, range) = (state.clone(), value_range.clone());
    Modifier
        .semantics_with(false, move |c: &mut SemanticsConfiguration| {
            if !enabled {
                c.disabled();
            }
            c.set_state_description(format_for_semantics(s.active_range_start()));
            let (state, range) = (s.clone(), range.clone());
            c.set_progress(move |target_value: f32| {
                let resolved_value = resolve_progress(target_value, *range.start(), *range.end(), state.start_steps());

                // This is to keep it consistent with AbsSeekbar.java: return false if no
                // change from current.
                if resolved_value == state.active_range_start() {
                    false
                } else {
                    let resolved_range = SliderRange::new(resolved_value, state.active_range_end());
                    let active_range = SliderRange::new(state.active_range_start(), state.active_range_end());
                    if resolved_range != active_range {
                        match state.on_value_change() {
                            Some(on_value_change) => on_value_change(resolved_range),
                            None => {
                                state.set_active_range_start(resolved_range.start());
                                state.set_active_range_end(resolved_range.end_inclusive());
                            }
                        }
                    }
                    if let Some(f) = state.on_value_change_finished() {
                        f();
                    }
                    true
                }
            });
        })
        .then(IncreaseHorizontalSemanticsBounds())
        .progress_semantics_with(state.active_range_start(), value_range, state.start_steps())
}

/// What a range slider's end thumb tells accessibility (Compose's private
/// `Modifier.rangeSliderEndThumbSemantics`): as a slider's, in the range from the start thumb.
fn range_slider_end_thumb_semantics(state: &RangeSliderState, enabled: bool) -> Modifier {
    let value_range = state.active_range_start()..=*state.value_range().end();
    let (s, range) = (state.clone(), value_range.clone());
    Modifier
        .semantics_with(false, move |c: &mut SemanticsConfiguration| {
            if !enabled {
                c.disabled();
            }
            c.set_state_description(format_for_semantics(s.active_range_end()));
            let (state, range) = (s.clone(), range.clone());
            c.set_progress(move |target_value: f32| {
                let resolved_value = resolve_progress(target_value, *range.start(), *range.end(), state.end_steps());

                // This is to keep it consistent with AbsSeekbar.java: return false if no
                // change from current.
                if resolved_value == state.active_range_end() {
                    false
                } else {
                    let resolved_range = SliderRange::new(state.active_range_start(), resolved_value);
                    let active_range = SliderRange::new(state.active_range_start(), state.active_range_end());
                    if resolved_range != active_range {
                        match state.on_value_change() {
                            Some(on_value_change) => on_value_change(resolved_range),
                            None => {
                                state.set_active_range_start(resolved_range.start());
                                state.set_active_range_end(resolved_range.end_inclusive());
                            }
                        }
                    }
                    if let Some(f) = state.on_value_change_finished() {
                        f();
                    }
                    true
                }
            });
        })
        .then(IncreaseHorizontalSemanticsBounds())
        .progress_semantics_with(state.active_range_end(), value_range, state.end_steps())
}

// ---------------------------------------------------------------------------------------------------------------
// Gestures

/// The key of a slider's pointer input (Compose's `pointerInput(state, interactionSource)`).
#[derive(PartialEq)]
struct TapKey(SliderState, MutableInteractionSource);

/// Cancels a press however the press gesture ends (Compose's `finally { press?.let { interactionSource.emit(
/// PressInteraction.Cancel(it)) } }`: an emit that never waits, so it is told even as the gesture is cancelled).
struct PressCancel {
    press: Cell<Option<PressInteraction::Press>>,
    interaction_source: MutableInteractionSource,
}

impl Drop for PressCancel {
    fn drop(&mut self) {
        if let Some(press) = self.press.take() {
            self.interaction_source.try_emit(PressInteraction::Cancel::new(press));
        }
    }
}

/// A slider's press and tap (Compose's private `Modifier.sliderTapModifier`): a press tells the source and moves
/// the thumb there at the next delta (a drag's first, or the tap's); a tap ends the change.
fn slider_tap_modifier(state: &SliderState, interaction_source: &MutableInteractionSource, enabled: bool) -> Modifier {
    if !enabled {
        return Modifier;
    }
    let (state, interaction_source) = (state.clone(), interaction_source.clone());
    Modifier.pointer_input(TapKey(state.clone(), interaction_source.clone()), move |scope: PointerInputScope| {
        let (press_state, tap_state, source) = (state.clone(), state.clone(), interaction_source.clone());
        async move {
            coroutine_scope(move |_| async move {
                scope
                    .detect_tap_gestures()
                    .on_press(move |gesture, offset: Offset| {
                        let (state, interaction_source) = (press_state.clone(), source.clone());
                        async move {
                            let guard = PressCancel { press: Cell::new(None), interaction_source };
                            let press = PressInteraction::Press::new(offset);
                            guard.press.set(Some(press));
                            guard.interaction_source.try_emit(press);
                            state.on_press(offset);
                            let success = gesture.try_await_release().await;
                            if success {
                                guard.interaction_source.try_emit(PressInteraction::Release::new(press));
                            } else {
                                guard.interaction_source.try_emit(PressInteraction::Cancel::new(press));
                            }
                            // Emit the release or cancel interaction
                            guard.press.set(None);
                        }
                    })
                    .on_tap(move |_| {
                        tap_state.dispatch_raw_delta(0.0);
                        tap_state.gesture_end_action();
                    })
                    .await
            })
            .await
        }
    })
}

/// The key of a range slider's pointer input (Compose's `pointerInput(startInteractionSource,
/// endInteractionSource, state)`).
#[derive(PartialEq)]
struct PressDragKey(MutableInteractionSource, MutableInteractionSource, RangeSliderState);

/// Which thumb a range slider's gesture holds, and what it told (Compose's locals of the gesture, which its
/// `finally` reads): however the gesture ends, a drag still on is ended and cancelled, a press still on cancelled.
struct RangeGesture {
    logic: Rc<RangeSliderLogic>,
    scope: CoroutineScope,
    active_drag_interaction: Cell<Option<DragInteraction::Start>>,
    dragging_start: Cell<bool>,
    press: Cell<Option<PressInteraction::Press>>,
}

impl Drop for RangeGesture {
    fn drop(&mut self) {
        let state = &self.logic.state;
        let dragging_start = self.dragging_start.get();
        if state.is_dragging() {
            // If we were dragging, clean up the state
            if let Some(it) = self.active_drag_interaction.get() {
                let interaction_source = self.logic.active_interaction(dragging_start).clone();
                self.scope.launch(async move { interaction_source.emit(DragInteraction::Cancel::new(it)).await });
            }
            state.gesture_end_action(dragging_start);
            state.set_is_dragging(false);
        }

        if let Some(it) = self.press.get() {
            let interaction_source = self.logic.active_interaction(dragging_start).clone();
            self.scope.launch(async move { interaction_source.emit(PressInteraction::Cancel::new(it)).await });
        }
    }
}

/// A range slider's press and drag (Compose's private `Modifier.rangeSliderPressDragModifier`): the nearer thumb
/// (the one whose side the press is on where they are together) is pressed; past the touch slop, a horizontal move
/// drags it (the press cancelled), a release taps it there, a vertical move cancels the press (a scroll's).
fn range_slider_press_drag_modifier(
    state: &RangeSliderState,
    start_interaction_source: &MutableInteractionSource,
    end_interaction_source: &MutableInteractionSource,
    enabled: bool,
) -> Modifier {
    if !enabled {
        return Modifier;
    }
    let (state, start_source, end_source) =
        (state.clone(), start_interaction_source.clone(), end_interaction_source.clone());
    Modifier.pointer_input(
        PressDragKey(start_source.clone(), end_source.clone(), state.clone()),
        move |scope: PointerInputScope| {
            let range_slider_logic =
                Rc::new(RangeSliderLogic::new(state.clone(), start_source.clone(), end_source.clone()));
            async move {
                coroutine_scope(move |cs| async move {
                    scope
                        .await_each_gesture(move |s| {
                            let (logic, cs) = (range_slider_logic.clone(), cs.clone());
                            async move { range_slider_gesture(s, logic, cs).await }
                        })
                        .await
                })
                .await
            }
        },
    )
}

/// One gesture of a range slider (the block of Compose's `awaitEachGesture`).
async fn range_slider_gesture(s: AwaitPointerEventScope, logic: Rc<RangeSliderLogic>, cs: CoroutineScope) {
    let state = logic.state.clone();
    let g = RangeGesture {
        logic: logic.clone(),
        scope: cs.clone(),
        active_drag_interaction: Cell::new(None),
        dragging_start: Cell::new(false),
        press: Cell::new(None),
    };
    // (Compose's `launch { interactionSource.emit(it) }`)
    let launch_emit = |source: &MutableInteractionSource, interaction: Arc<dyn Interaction>| {
        let source = source.clone();
        cs.launch(async move { source.emit(interaction).await });
    };

    let down = s.await_first_down_with(false, kompoz_ui::input::PointerEventPass::Main).await;

    let pos_x = if state.is_rtl() { state.total_width() as f32 - down.position.x } else { down.position.x };
    let compare = logic.compare_offsets(pos_x);

    g.dragging_start.set(if compare != 0 { compare < 0 } else { state.raw_offset_start() > pos_x });
    let dragging_start = g.dragging_start.get();

    let interaction_source = logic.active_interaction(dragging_start).clone();
    let press = PressInteraction::Press::new(down.position);
    g.press.set(Some(press));
    launch_emit(&interaction_source, Arc::new(press));

    let mut drag: Option<PointerInputChange>;
    let mut over_slop = Offset::Zero;
    let pointer_slop = pointer_slop(&s.view_configuration(), down.r#type);

    loop {
        drag = s.await_pointer_event().await.changes.first().cloned();
        if let Some(d) = &drag {
            over_slop = over_slop + d.position_change();
        }
        let go_on =
            drag.as_ref().is_some_and(|d| d.pressed) && over_slop.get_distance_squared() < pointer_slop * pointer_slop;
        if !go_on {
            break;
        }
    }

    if drag.is_some() && over_slop.x.abs() > over_slop.y.abs() {
        // The press is converted to a drag, so we cancel the press interaction
        if let Some(it) = g.press.take() {
            launch_emit(&interaction_source, Arc::new(PressInteraction::Cancel::new(it)));
        }

        let interaction = DragInteraction::Start::new();
        g.active_drag_interaction.set(Some(interaction));
        launch_emit(&interaction_source, Arc::new(interaction));

        // Apply the slop from the initial drag detection
        let initial_offset = pos_x - if dragging_start { state.raw_offset_start() } else { state.raw_offset_end() };
        let total_drag_offset = initial_offset + if state.is_rtl() { -over_slop.x } else { over_slop.x };
        state.on_drag(dragging_start, total_drag_offset);

        // The main drag block now handles all subsequent movement.
        state.set_is_dragging(true);
        let drag_state = state.clone();
        let success = s
            .horizontal_drag(down.id, move |change| {
                let delta_x = change.position_change().x;
                drag_state.on_drag(dragging_start, if drag_state.is_rtl() { -delta_x } else { delta_x });
                change.consume();
            })
            .await;

        state.set_is_dragging(false);

        state.gesture_end_action(dragging_start);
        if success {
            launch_emit(&interaction_source, Arc::new(DragInteraction::Stop::new(interaction)));
        } else {
            launch_emit(&interaction_source, Arc::new(DragInteraction::Cancel::new(interaction)));
        }
        g.active_drag_interaction.set(None);
    } else if drag.as_ref().is_some_and(|d| !d.pressed) {
        // The press is completed, so emit a release.
        if let Some(it) = g.press.take() {
            launch_emit(&interaction_source, Arc::new(PressInteraction::Release::new(it)));
        }

        // Perform the tap action to update the slider value
        let offset = pos_x - if dragging_start { state.raw_offset_start() } else { state.raw_offset_end() };
        state.on_drag(dragging_start, offset);
        state.gesture_end_action(dragging_start);
    } else {
        // Gesture is not a horizontal drag and pointer is still down
        // (e.g.,
        // vertical scroll)
        // Cancel the press to dismiss the ripple and allow scrolling
        if let Some(it) = g.press.take() {
            launch_emit(&interaction_source, Arc::new(PressInteraction::Cancel::new(it)));
        }
    }
}

/// Which thumb a range slider's gesture is about (Compose's private class `RangeSliderLogic`).
struct RangeSliderLogic {
    state: RangeSliderState,
    start_interaction_source: MutableInteractionSource,
    end_interaction_source: MutableInteractionSource,
}

impl RangeSliderLogic {
    fn new(
        state: RangeSliderState,
        start_interaction_source: MutableInteractionSource,
        end_interaction_source: MutableInteractionSource,
    ) -> RangeSliderLogic {
        RangeSliderLogic { state, start_interaction_source, end_interaction_source }
    }

    /// The source of the thumb dragged (Compose's `activeInteraction`).
    fn active_interaction(&self, dragging_start: bool) -> &MutableInteractionSource {
        if dragging_start { &self.start_interaction_source } else { &self.end_interaction_source }
    }

    /// Which thumb `event_x` is nearer: negative the start, positive the end, 0 both (Compose's `compareOffsets`).
    fn compare_offsets(&self, event_x: f32) -> i32 {
        let diff_start = (self.state.raw_offset_start() - event_x).abs();
        let diff_end = (self.state.raw_offset_end() - event_x).abs();
        // (Kotlin's Float.compareTo: a total order)
        match diff_start.total_cmp(&diff_end) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    /// Moves the thumb to `pos_x` and tells its source `interaction` (Compose's `captureThumb`; unused in Compose's
    /// Slider.kt too).
    #[allow(dead_code)]
    fn capture_thumb(
        &self,
        dragging_start: bool,
        pos_x: f32,
        interaction: Arc<dyn Interaction>,
        scope: &CoroutineScope,
    ) {
        self.state.on_drag(
            dragging_start,
            pos_x - if dragging_start { self.state.raw_offset_start() } else { self.state.raw_offset_end() },
        );
        let source = self.active_interaction(dragging_start).clone();
        scope.launch(async move { source.emit(interaction).await });
    }
}

/// Waits past the horizontal pointer slop, consuming the change that reaches it: that change and how far past the
/// slop it went, none if the pointer was lifted or another took it (Compose's private
/// `AwaitPointerEventScope.awaitSlop`; unused in Compose's Slider.kt too).
#[allow(dead_code)]
async fn await_slop(
    s: &AwaitPointerEventScope,
    id: PointerId,
    r#type: PointerType,
) -> Option<(PointerInputChange, f32)> {
    let initial_delta = Cell::new(0.0f32);
    let post_pointer_slop = |pointer_input: &PointerInputChange, offset: f32| {
        pointer_input.consume();
        initial_delta.set(offset);
    };
    let after_slop_result = await_horizontal_pointer_slop_or_cancellation(s, id, r#type, post_pointer_slop).await;
    after_slop_result.map(|it| (it, initial_delta.get()))
}

// ---------------------------------------------------------------------------------------------------------------
// The sizes

// Internal to be referred to in tests
/// A track's thickness (Compose's internal `TrackHeight`): 16 dp.
pub(crate) const TrackHeight: Dp = crate::tokens::SliderTokens::InactiveTrackHeight;
/// A thumb's width (Compose's internal `ThumbWidth`): 4 dp.
pub(crate) const ThumbWidth: Dp = crate::tokens::SliderTokens::HandleWidth;
const ThumbHeight: Dp = crate::tokens::SliderTokens::HandleHeight;
pub(crate) const ThumbSize: DpSize = DpSize { width: ThumbWidth, height: ThumbHeight };
const VerticalThumbSize: DpSize = DpSize { width: ThumbHeight, height: ThumbWidth };
pub(crate) const ThumbTrackGapSize: Dp = crate::tokens::SliderTokens::ActiveHandleLeadingSpace;
pub(crate) const TrackInsideCornerSize: Dp = Dp(2.0);

/// The corner size a track tells its slider, so that a stepped thumb stands on its tick (Compose's internal
/// `CornerSizeAlignmentLine`; the smallest of several).
pub(crate) const CornerSizeAlignmentLine: VerticalAlignmentLine =
    VerticalAlignmentLine::new("CornerSizeAlignmentLine", min);

fn min(a: i32, b: i32) -> i32 {
    a.min(b)
}
