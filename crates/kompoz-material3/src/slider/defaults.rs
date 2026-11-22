//! The sliders' defaults, as Compose's Slider.kt has them: [`SliderDefaults`] (the colors, the thumb — a 4 dp bar,
//! half as thick while pressed, focused or dragged —, the tracks — the active and the inactive stretches kept apart
//! from the thumb, their inside corners small, stop indicators at their ends, the steps' ticks —, Expressive's
//! tracks with a corner size and the centered track), [`SliderColors`], and the private thumb and track drawing.

// (the deprecated SliderPositions and its track are defined here, as Compose's are)
#![allow(deprecated)]

use super::state::{RangeSliderState, SliderState};
use super::{CornerSizeAlignmentLine, ThumbSize, ThumbTrackGapSize, TrackHeight, TrackInsideCornerSize};
use crate::color_scheme::ColorScheme;
use crate::material_theme::MaterialTheme;
use crate::tokens::SliderTokens;
use kompoz_coroutines::LaunchedEffect;
use kompoz_foundation::Canvas;
use kompoz_foundation::gestures::Orientation;
use kompoz_foundation::interaction::{
    DragInteraction, FocusInteraction, Interaction, InteractionId, MutableInteractionSource, PressInteraction,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    Color, CornerRadius, Dp, DpSize, LayoutDirection, Offset, Path, PathDirection, Rect, RoundRect, Size, StrokeCap,
    lerp, lerp_offset,
};
use kompoz_runtime::{
    KotlinProperties, kompozable, kompozable_block, mutable_state_list_of, named, named_args, remember,
};
use kompoz_ui::Modifier;
use kompoz_ui::draw::{DrawScope, PointMode};
use kompoz_ui::layout::AlignmentLine;
use kompoz_ui::platform::PointerIcon;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// What a track draws at its stop indicators' places (Compose's `drawStopIndicator: (DrawScope.(Offset) -> Unit)`).
pub type DrawStopIndicator = Rc<dyn Fn(&mut DrawScope, Offset)>;

/// What a track draws at its ticks' places, in the tick's color (Compose's `drawTick: DrawScope.(Offset, Color) ->
/// Unit`).
pub type DrawTick = Rc<dyn Fn(&mut DrawScope, Offset, Color)>;

/// The sliders' defaults (Compose's SliderDefaults): their colors, thumbs and tracks. A value:
/// `SliderDefaults.colors()`, `SliderDefaults.Thumb(source)`.
pub struct SliderDefaults;

#[named]
#[allow(non_snake_case)]
impl SliderDefaults {
    /// The colors of a slider (Compose's `SliderDefaults.colors()` and `colors(thumbColor, …)`): the scheme's — a
    /// primary thumb and active track, a secondary container inactive track, the ticks in the other stretch's
    /// color; disabled, the surface's content color faded — each given one in its place.
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] thumb_color: Color,
        #[default(Color::Unspecified)] active_track_color: Color,
        #[default(Color::Unspecified)] active_tick_color: Color,
        #[default(Color::Unspecified)] inactive_track_color: Color,
        #[default(Color::Unspecified)] inactive_tick_color: Color,
        #[default(Color::Unspecified)] disabled_thumb_color: Color,
        #[default(Color::Unspecified)] disabled_active_track_color: Color,
        #[default(Color::Unspecified)] disabled_active_tick_color: Color,
        #[default(Color::Unspecified)] disabled_inactive_track_color: Color,
        #[default(Color::Unspecified)] disabled_inactive_tick_color: Color,
    ) -> SliderColors {
        named_args!(default_slider_colors(&MaterialTheme::color_scheme()).copy(
            thumb_color = thumb_color,
            active_track_color = active_track_color,
            active_tick_color = active_tick_color,
            inactive_track_color = inactive_track_color,
            inactive_tick_color = inactive_tick_color,
            disabled_thumb_color = disabled_thumb_color,
            disabled_active_track_color = disabled_active_track_color,
            disabled_active_tick_color = disabled_active_tick_color,
            disabled_inactive_track_color = disabled_inactive_track_color,
            disabled_inactive_tick_color = disabled_inactive_tick_color
        ))
    }

    /// The thumb of a slider (Compose's `SliderDefaults.Thumb(interactionSource, …)`): a bar of `thumb_size` (4 by
    /// 44 dp) in the colors' thumb color, half as wide while `interaction_source` says it is pressed, focused or
    /// dragged; a hand pointer over it, which tells its hovering to the source.
    #[track_caller]
    pub fn Thumb(
        &self,
        interaction_source: MutableInteractionSource,
        #[default(Modifier)] modifier: Modifier,
        #[default(SliderDefaults.colors())] colors: SliderColors,
        #[default(true)] enabled: bool,
        #[default(ThumbSize)] thumb_size: DpSize,
    ) {
        kompozable_block!({
            Thumb(interaction_source, modifier, colors, enabled, thumb_size, false);
        })
    }

    /// The thumb of a slider of `slider_state` (Compose's Expressive `SliderDefaults.Thumb(interactionSource,
    /// sliderState, …)`): as [`Thumb`](Self::Thumb), lying across a vertical slider (half as tall while held).
    ///
    /// Kotlin: `Thumb(interactionSource: MutableInteractionSource, sliderState: SliderState)`
    #[track_caller]
    pub fn StateThumb(
        &self,
        interaction_source: MutableInteractionSource,
        slider_state: SliderState,
        #[default(Modifier)] modifier: Modifier,
        #[default(SliderDefaults.colors())] colors: SliderColors,
        #[default(true)] enabled: bool,
        #[default(ThumbSize)] thumb_size: DpSize,
    ) {
        kompozable_block!({
            Thumb(
                interaction_source,
                modifier,
                colors,
                enabled,
                thumb_size,
                slider_state.orientation() == Orientation::Vertical,
            );
        })
    }

    /// The track of a slider at `slider_positions` (Compose's deprecated `SliderDefaults.Track(sliderPositions,
    /// …)`): a 16 dp line with round caps, the active range over it, the ticks as dots.
    ///
    /// Kotlin: `Track(sliderPositions: SliderPositions)`
    #[deprecated(note = "Use version that supports slider state")]
    #[allow(deprecated)]
    #[track_caller]
    pub fn PositionsTrack(
        &self,
        slider_positions: super::state::SliderPositions,
        #[default(Modifier)] modifier: Modifier,
        #[default(SliderDefaults.colors())] colors: SliderColors,
        #[default(true)] enabled: bool,
    ) {
        kompozable_block!({
            let inactive_track_color = colors.track_color(enabled, false);
            let active_track_color = colors.track_color(enabled, true);
            let inactive_tick_color = colors.tick_color(enabled, false);
            let active_tick_color = colors.tick_color(enabled, true);
            Canvas(modifier.fill_max_width().height(TrackHeight), move |d: &mut DrawScope| {
                let is_rtl = d.layout_direction() == LayoutDirection::Rtl;
                let center = d.center();
                let size = d.size();
                let slider_left = Offset::new(0.0, center.y);
                let slider_right = Offset::new(size.width, center.y);
                let slider_start = if is_rtl { slider_right } else { slider_left };
                let slider_end = if is_rtl { slider_left } else { slider_right };
                let tick_size = d.to_px(SliderDefaults::TickSize);
                let track_stroke_width = d.to_px(TrackHeight);
                named_args!(d.draw_line(
                    inactive_track_color,
                    slider_start,
                    slider_end,
                    stroke_width = track_stroke_width,
                    cap = StrokeCap::Round
                ));
                let active_range = slider_positions.active_range();
                let slider_value_end =
                    Offset::new(slider_start.x + (slider_end.x - slider_start.x) * *active_range.end(), center.y);
                let slider_value_start =
                    Offset::new(slider_start.x + (slider_end.x - slider_start.x) * *active_range.start(), center.y);
                named_args!(d.draw_line(
                    active_track_color,
                    slider_value_start,
                    slider_value_end,
                    stroke_width = track_stroke_width,
                    cap = StrokeCap::Round
                ));
                // (Kotlin's `groupBy { outside }`: the groups in the order their first tick comes)
                let ticks = slider_positions.tick_fractions();
                let outside = |it: f32| it > *active_range.end() || it < *active_range.start();
                let mut groups: Vec<(bool, Vec<Offset>)> = Vec::new();
                for &it in &ticks {
                    let point = Offset::new(lerp_offset(slider_start, slider_end, it).x, center.y);
                    match groups.iter_mut().find(|g| g.0 == outside(it)) {
                        Some(g) => g.1.push(point),
                        None => groups.push((outside(it), vec![point])),
                    }
                }
                for (outside_fraction, list) in groups {
                    named_args!(d.draw_points(
                        &list,
                        PointMode::Points,
                        if outside_fraction { inactive_tick_color } else { active_tick_color },
                        stroke_width = tick_size,
                        cap = StrokeCap::Round
                    ));
                }
            });
        })
    }

    /// The track of a slider of `slider_state` (Compose's `SliderDefaults.Track(sliderState, …)`): a 16 dp bar, the
    /// active stretch up to the value and the inactive one after it, each kept `thumb_track_gap_size` from the
    /// thumb, their ends there rounded by `track_inside_corner_size`; `draw_stop_indicator` (none: Kotlin's
    /// default, a dot of [`TrackStopIndicatorSize`](Self::TrackStopIndicatorSize) in the active track color;
    /// `Some(None)`: nothing) at the inactive stretch's end, `draw_tick` at each step's tick (in the active or the
    /// inactive tick color).
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn Track(
        &self,
        slider_state: SliderState,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SliderDefaults.colors())] colors: SliderColors,
        #[default(None)] draw_stop_indicator: Option<Option<DrawStopIndicator>>,
        #[default(default_draw_tick())] draw_tick: DrawTick,
        #[default(ThumbTrackGapSize)] thumb_track_gap_size: Dp,
        #[default(TrackInsideCornerSize)] track_inside_corner_size: Dp,
    ) {
        kompozable_block!({
            TrackImpl(
                slider_state,
                Dp::Unspecified,
                modifier,
                enabled,
                colors,
                resolve_draw_stop_indicator(draw_stop_indicator, colors, enabled),
                draw_tick,
                thumb_track_gap_size,
                track_inside_corner_size,
                false,
                false,
            );
        })
    }

    /// The track of a slider of `slider_state` with corners of `track_corner_size` (Compose's Expressive
    /// `SliderDefaults.Track(sliderState, trackCornerSize, …)`; [`Dp::Unspecified`]: half its thickness): as
    /// [`Track`](Self::Track), its stretches shrinking their corners as they get short.
    ///
    /// Kotlin: `Track(sliderState: SliderState, trackCornerSize: Dp)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn CornerSizeTrack(
        &self,
        slider_state: SliderState,
        track_corner_size: Dp,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SliderDefaults.colors())] colors: SliderColors,
        #[default(None)] draw_stop_indicator: Option<Option<DrawStopIndicator>>,
        #[default(default_draw_tick())] draw_tick: DrawTick,
        #[default(ThumbTrackGapSize)] thumb_track_gap_size: Dp,
        #[default(TrackInsideCornerSize)] track_inside_corner_size: Dp,
    ) {
        kompozable_block!({
            TrackImpl(
                slider_state,
                track_corner_size,
                modifier,
                enabled,
                colors,
                resolve_draw_stop_indicator(draw_stop_indicator, colors, enabled),
                draw_tick,
                thumb_track_gap_size,
                track_inside_corner_size,
                true,
                false,
            );
        })
    }

    /// A track whose active stretch runs from its middle to the value (Compose's Expressive
    /// `SliderDefaults.CenteredTrack(sliderState, …)`): for a range around 0, a stop indicator at each end.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn CenteredTrack(
        &self,
        slider_state: SliderState,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SliderDefaults.colors())] colors: SliderColors,
        #[default(None)] draw_stop_indicator: Option<Option<DrawStopIndicator>>,
        #[default(default_draw_tick())] draw_tick: DrawTick,
        #[default(ThumbTrackGapSize)] thumb_track_gap_size: Dp,
        #[default(TrackInsideCornerSize)] track_inside_corner_size: Dp,
        #[default(Dp::Unspecified)] track_corner_size: Dp,
    ) {
        kompozable_block!({
            TrackImpl(
                slider_state,
                track_corner_size,
                modifier,
                enabled,
                colors,
                resolve_draw_stop_indicator(draw_stop_indicator, colors, enabled),
                draw_tick,
                thumb_track_gap_size,
                track_inside_corner_size,
                true,
                true,
            );
        })
    }

    /// The track of a range slider of `range_slider_state` (Compose's `SliderDefaults.Track(rangeSliderState, …)`):
    /// the active stretch between the thumbs, an inactive one on each side, each kept `thumb_track_gap_size` from
    /// the thumbs; a stop indicator at each end, the ticks of the steps.
    ///
    /// Kotlin: `Track(rangeSliderState: RangeSliderState)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn RangeTrack(
        &self,
        range_slider_state: RangeSliderState,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SliderDefaults.colors())] colors: SliderColors,
        #[default(None)] draw_stop_indicator: Option<Option<DrawStopIndicator>>,
        #[default(default_draw_tick())] draw_tick: DrawTick,
        #[default(ThumbTrackGapSize)] thumb_track_gap_size: Dp,
        #[default(TrackInsideCornerSize)] track_inside_corner_size: Dp,
    ) {
        kompozable_block!({
            RangeTrackImpl(
                range_slider_state,
                Dp::Unspecified,
                modifier,
                enabled,
                colors,
                resolve_draw_stop_indicator(draw_stop_indicator, colors, enabled),
                draw_tick,
                thumb_track_gap_size,
                track_inside_corner_size,
            );
        })
    }

    /// The track of a range slider with corners of `track_corner_size` (Compose's Expressive
    /// `SliderDefaults.Track(rangeSliderState, trackCornerSize, …)`).
    ///
    /// Kotlin: `Track(rangeSliderState: RangeSliderState, trackCornerSize: Dp)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn RangeCornerSizeTrack(
        &self,
        range_slider_state: RangeSliderState,
        track_corner_size: Dp,
        #[default(Modifier)] modifier: Modifier,
        #[default(true)] enabled: bool,
        #[default(SliderDefaults.colors())] colors: SliderColors,
        #[default(None)] draw_stop_indicator: Option<Option<DrawStopIndicator>>,
        #[default(default_draw_tick())] draw_tick: DrawTick,
        #[default(ThumbTrackGapSize)] thumb_track_gap_size: Dp,
        #[default(TrackInsideCornerSize)] track_inside_corner_size: Dp,
    ) {
        kompozable_block!({
            RangeTrackImpl(
                range_slider_state,
                track_corner_size,
                modifier,
                enabled,
                colors,
                resolve_draw_stop_indicator(draw_stop_indicator, colors, enabled),
                draw_tick,
                thumb_track_gap_size,
                track_inside_corner_size,
            );
        })
    }

    /// Draws a stop indicator: a dot of `size` at `offset` in `color` (Compose's `DrawScope.drawStopIndicator(offset,
    /// size, color)`, a member extension of SliderDefaults: the draw scope is given first here).
    ///
    /// Kotlin: `drawStopIndicator(drawScope: DrawScope, offset: Offset, size: Dp, color: Color)`
    pub fn draw_stop_indicator(&self, draw_scope: &mut DrawScope, offset: Offset, size: Dp, color: Color) {
        let radius = draw_scope.to_px(size) / 2.0;
        named_args!(draw_scope.draw_circle(color, center = offset, radius = radius));
    }
}

impl SliderDefaults {
    /// The size of a track's stop indicators (Compose's `SliderDefaults.TrackStopIndicatorSize`): 4 dp.
    pub const TrackStopIndicatorSize: Dp = SliderTokens::StopIndicatorSize;

    /// The size of a track's ticks (Compose's `SliderDefaults.TickSize`): 4 dp.
    pub const TickSize: Dp = SliderTokens::StopIndicatorSize;
}

/// The default tick drawing (Compose's default `drawTick`: a stop indicator of [`SliderDefaults::TickSize`]).
pub(crate) fn default_draw_tick() -> DrawTick {
    Rc::new(|d: &mut DrawScope, offset: Offset, color: Color| {
        SliderDefaults.draw_stop_indicator(d, offset, SliderDefaults::TickSize, color)
    })
}

/// Kotlin's default of `drawStopIndicator` where none was given (a dot of TrackStopIndicatorSize in the active
/// track color), Kotlin's `null` (`Some(None)`) none, a lambda itself.
fn resolve_draw_stop_indicator(
    draw_stop_indicator: Option<Option<DrawStopIndicator>>,
    colors: SliderColors,
    enabled: bool,
) -> Option<DrawStopIndicator> {
    draw_stop_indicator.unwrap_or_else(|| {
        let color = colors.track_color(enabled, true);
        Some(Rc::new(move |d: &mut DrawScope, offset: Offset| {
            SliderDefaults.draw_stop_indicator(d, offset, SliderDefaults::TrackStopIndicatorSize, color)
        }))
    })
}

/// A scheme's default slider colors (Compose's internal `ColorScheme.defaultSliderColors`; its cache isn't ported:
/// the colors are plain values, made where asked).
pub(crate) fn default_slider_colors(scheme: &ColorScheme) -> SliderColors {
    SliderColors(
        scheme.from_token(SliderTokens::HandleColor),
        scheme.from_token(SliderTokens::ActiveTrackColor),
        scheme.from_token(SliderTokens::InactiveTrackColor),
        scheme.from_token(SliderTokens::InactiveTrackColor),
        scheme.from_token(SliderTokens::ActiveTrackColor),
        named_args!(
            scheme.from_token(SliderTokens::DisabledHandleColor).copy(alpha = SliderTokens::DisabledHandleOpacity)
        )
        .composite_over(scheme.surface),
        named_args!(
            scheme
                .from_token(SliderTokens::DisabledActiveTrackColor)
                .copy(alpha = SliderTokens::DisabledActiveTrackOpacity)
        ),
        named_args!(
            scheme
                .from_token(SliderTokens::DisabledInactiveTrackColor)
                .copy(alpha = SliderTokens::DisabledInactiveTrackOpacity)
        ),
        named_args!(
            scheme
                .from_token(SliderTokens::DisabledInactiveTrackColor)
                .copy(alpha = SliderTokens::DisabledInactiveTrackOpacity)
        ),
        named_args!(
            scheme
                .from_token(SliderTokens::DisabledActiveTrackColor)
                .copy(alpha = SliderTokens::DisabledActiveTrackOpacity)
        ),
    )
}

// ---------------------------------------------------------------------------------------------------------------
// The tracks' drawing

/// A slider's track (Compose's private `TrackImpl` of a SliderState): a canvas across the slider (down a vertical
/// one, flipped when reversed) telling its corner size to the slider (`CornerSizeAlignmentLine`), drawing the
/// stretches.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn TrackImpl(
    slider_state: SliderState,
    track_corner_size: Dp,
    modifier: Modifier,
    enabled: bool,
    colors: SliderColors,
    draw_stop_indicator: Option<DrawStopIndicator>,
    draw_tick: DrawTick,
    thumb_track_gap_size: Dp,
    track_inside_corner_size: Dp,
    enable_corner_shrinking: bool,
    is_centered: bool,
) {
    let inactive_track_color = colors.track_color(enabled, false);
    let active_track_color = colors.track_color(enabled, true);
    let inactive_tick_color = colors.tick_color(enabled, false);
    let active_tick_color = colors.tick_color(enabled, true);
    let vertical = slider_state.orientation() == Orientation::Vertical;
    let sized = if vertical {
        let m = modifier.width(TrackHeight).fill_max_height();
        if slider_state.reverse_vertical_direction() { m.scale_xy(1.0, -1.0) } else { m }
    } else {
        modifier.fill_max_width().height(TrackHeight)
    };
    let layout_state = slider_state.clone();
    Canvas(
        sized.then(Modifier.layout(move |scope, measurable, constraints| {
            let placeable = measurable.measure(constraints);
            let corner_size = if track_corner_size.is_unspecified() {
                if layout_state.orientation() == Orientation::Vertical {
                    placeable.width() / 2
                } else {
                    placeable.height() / 2
                }
            } else {
                scope.round_to_px(track_corner_size)
            };
            let (width, height) = (placeable.width(), placeable.height());
            named_args!(scope.layout(
                width,
                height,
                alignment_lines = vec![(AlignmentLine::from(CornerSizeAlignmentLine), corner_size)],
                placement_block = move |p| p.place(&placeable, 0, 0)
            ))
        })),
        move |d: &mut DrawScope| {
            let corner_size: f32 = if track_corner_size.is_unspecified() {
                if slider_state.orientation() == Orientation::Vertical {
                    d.size().width / 2.0
                } else {
                    d.size().height / 2.0
                }
            } else {
                d.to_px(track_corner_size)
            };
            let track = TrackDrawing {
                tick_fractions: slider_state.tick_fractions(),
                active_range_start: 0.0,
                active_range_end: slider_state.coerced_value_as_fraction(),
                inactive_track_color,
                active_track_color,
                inactive_tick_color,
                active_tick_color,
                start_thumb_width: Dp(0.0),
                start_thumb_height: Dp(0.0),
                end_thumb_width: d.to_dp(slider_state.thumb_width() as f32),
                end_thumb_height: d.to_dp(slider_state.thumb_height() as f32),
                thumb_track_gap_size,
                track_inside_corner_size,
                track_corner_size: d.to_dp(corner_size),
                draw_stop_indicator: draw_stop_indicator.as_ref(),
                draw_tick: &draw_tick,
                is_range_slider: false,
                enable_corner_shrinking,
                orientation: slider_state.orientation(),
                is_centered,
            };
            draw_track(d, &track);
        },
    );
}

/// A range slider's track (Compose's private `TrackImpl` of a RangeSliderState).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn RangeTrackImpl(
    range_slider_state: RangeSliderState,
    track_corner_size: Dp,
    modifier: Modifier,
    enabled: bool,
    colors: SliderColors,
    draw_stop_indicator: Option<DrawStopIndicator>,
    draw_tick: DrawTick,
    thumb_track_gap_size: Dp,
    track_inside_corner_size: Dp,
) {
    let inactive_track_color = colors.track_color(enabled, false);
    let active_track_color = colors.track_color(enabled, true);
    let inactive_tick_color = colors.tick_color(enabled, false);
    let active_tick_color = colors.tick_color(enabled, true);
    Canvas(
        modifier.fill_max_width().height(TrackHeight).layout(move |scope, measurable, constraints| {
            let placeable = measurable.measure(constraints);
            let track_corner_size = placeable.height() / 2;
            let (width, height) = (placeable.width(), placeable.height());
            named_args!(scope.layout(
                width,
                height,
                alignment_lines = vec![(AlignmentLine::from(CornerSizeAlignmentLine), track_corner_size)],
                placement_block = move |p| p.place(&placeable, 0, 0)
            ))
        }),
        move |d: &mut DrawScope| {
            let corner_size: f32 =
                if track_corner_size.is_unspecified() { d.size().height / 2.0 } else { d.to_px(track_corner_size) };
            let state = &range_slider_state;
            let track = TrackDrawing {
                tick_fractions: state.tick_fractions(),
                active_range_start: state.coerced_active_range_start_as_fraction(),
                active_range_end: state.coerced_active_range_end_as_fraction(),
                inactive_track_color,
                active_track_color,
                inactive_tick_color,
                active_tick_color,
                start_thumb_width: d.to_dp(state.start_thumb_width()),
                start_thumb_height: d.to_dp(state.start_thumb_height()),
                end_thumb_width: d.to_dp(state.end_thumb_width()),
                end_thumb_height: d.to_dp(state.end_thumb_height()),
                thumb_track_gap_size,
                track_inside_corner_size,
                track_corner_size: d.to_dp(corner_size),
                draw_stop_indicator: draw_stop_indicator.as_ref(),
                draw_tick: &draw_tick,
                is_range_slider: true,
                enable_corner_shrinking: false,
                orientation: Orientation::Horizontal,
                is_centered: false,
            };
            draw_track(d, &track);
        },
    );
}

/// What [`draw_track`] draws (Compose's private `drawTrack`'s parameters).
struct TrackDrawing<'a> {
    tick_fractions: &'a [f32],
    active_range_start: f32,
    active_range_end: f32,
    inactive_track_color: Color,
    active_track_color: Color,
    inactive_tick_color: Color,
    active_tick_color: Color,
    start_thumb_width: Dp,
    start_thumb_height: Dp,
    end_thumb_width: Dp,
    end_thumb_height: Dp,
    thumb_track_gap_size: Dp,
    track_inside_corner_size: Dp,
    track_corner_size: Dp,
    draw_stop_indicator: Option<&'a DrawStopIndicator>,
    draw_tick: &'a DrawTick,
    is_range_slider: bool,
    enable_corner_shrinking: bool,
    orientation: Orientation,
    is_centered: bool,
}

/// Whether `value` is in `start..=end` (Kotlin's `in` of a float range).
fn contains(start: f32, end: f32, value: f32) -> bool {
    start <= value && value <= end
}

/// The stretches, the stop indicators and the ticks (Compose's private `DrawScope.drawTrack`).
fn draw_track(d: &mut DrawScope, t: &TrackDrawing) {
    let is_vertical = t.orientation == Orientation::Vertical;
    let is_rtl = d.layout_direction() == LayoutDirection::Rtl;
    let is_rtl_horizontal = is_rtl && !is_vertical;
    let corner_size = d.to_px(t.track_corner_size);
    let size = d.size();
    let center = d.center();
    let slider_start = 0.0f32;
    let slider_end = if is_vertical { size.height } else { size.width };
    let tick_fractions = t.tick_fractions;

    let first = tick_fractions.first().copied();
    let last = tick_fractions.last().copied();
    let is_start_on_first_or_last_step = Some(t.active_range_start) == first || Some(t.active_range_start) == last;
    let is_end_on_first_or_last_step = Some(t.active_range_end) == first || Some(t.active_range_end) == last;
    let slider_value_end = if !tick_fractions.is_empty() && !is_end_on_first_or_last_step {
        slider_start + (slider_end - slider_start - corner_size * 2.0) * t.active_range_end + corner_size
    } else {
        slider_start + (slider_end - slider_start) * t.active_range_end
    };
    let slider_value_start = if !tick_fractions.is_empty() && !is_start_on_first_or_last_step {
        slider_start + (slider_end - slider_start - corner_size * 2.0) * t.active_range_start + corner_size
    } else {
        slider_start + (slider_end - slider_start) * t.active_range_start
    };

    let inside_corner_size = d.to_px(t.track_inside_corner_size);
    let mut start_gap = 0.0f32;
    let mut end_gap = 0.0f32;
    if t.thumb_track_gap_size > Dp(0.0) {
        if is_vertical {
            start_gap = d.to_px(t.start_thumb_height) / 2.0 + d.to_px(t.thumb_track_gap_size);
            end_gap = d.to_px(t.end_thumb_height) / 2.0 + d.to_px(t.thumb_track_gap_size);
        } else {
            start_gap = d.to_px(t.start_thumb_width) / 2.0 + d.to_px(t.thumb_track_gap_size);
            end_gap = d.to_px(t.end_thumb_width) / 2.0 + d.to_px(t.thumb_track_gap_size);
        }
    }
    let center_axis = if is_vertical { center.y } else { center.x };

    // inactive track (centered or range slider)
    let mut range_inactive_track_threshold = slider_start + start_gap;
    if !t.enable_corner_shrinking || !tick_fractions.is_empty() {
        range_inactive_track_threshold += corner_size;
    }
    let adjusted_slider_value_end = if t.is_centered { slider_value_end.min(center_axis) } else { slider_value_start };
    if (t.is_centered || t.is_range_slider) && adjusted_slider_value_end > range_inactive_track_threshold {
        let start_corner_radius = if is_rtl_horizontal { inside_corner_size } else { corner_size };
        let end_corner_radius = if is_rtl_horizontal { corner_size } else { inside_corner_size };
        let start = slider_start;
        let end = adjusted_slider_value_end - start_gap;
        let track_offset = if is_rtl_horizontal { Offset::new(size.width - end, 0.0) } else { Offset::new(0.0, 0.0) };
        let track_size =
            if is_vertical { Size::new(size.width, end - start) } else { Size::new(end - start, size.height) };
        draw_track_path(
            d,
            t.orientation,
            track_offset,
            track_size,
            t.inactive_track_color,
            start_corner_radius,
            end_corner_radius,
        );
        let stop_indicator_offset = if is_vertical {
            Offset::new(center.x, start + corner_size)
        } else if is_rtl {
            Offset::new(size.width - start - corner_size, center.y)
        } else {
            Offset::new(start + corner_size, center.y)
        };
        if let Some(f) = t.draw_stop_indicator {
            f(d, stop_indicator_offset);
        }
    }
    // inactive track
    let mut inactive_track_threshold = slider_end - end_gap;
    if !t.enable_corner_shrinking || !tick_fractions.is_empty() {
        inactive_track_threshold -= corner_size;
    }
    let adjusted_slider_value_start = if t.is_centered { slider_value_end.max(center_axis) } else { slider_value_end };
    if adjusted_slider_value_start < inactive_track_threshold {
        let start_corner_radius = if is_rtl_horizontal { corner_size } else { inside_corner_size };
        let end_corner_radius = if is_rtl_horizontal { inside_corner_size } else { corner_size };
        let start = adjusted_slider_value_start + end_gap;
        let end = slider_end;
        let inactive_track_width = end - start;
        let track_offset = if is_vertical {
            Offset::new(0.0, start)
        } else if is_rtl {
            Offset::new(0.0, 0.0)
        } else {
            Offset::new(start, 0.0)
        };
        let track_size = if is_vertical {
            Size::new(size.width, inactive_track_width)
        } else if is_rtl && !t.is_range_slider {
            Size::new(size.width - start, size.height)
        } else {
            Size::new(inactive_track_width, size.height)
        };
        draw_track_path(
            d,
            t.orientation,
            track_offset,
            track_size,
            t.inactive_track_color,
            start_corner_radius,
            end_corner_radius,
        );
        let stop_indicator_offset = if is_vertical {
            Offset::new(center.x, end - corner_size)
        } else if is_rtl {
            Offset::new(corner_size, center.y)
        } else {
            Offset::new(end - corner_size, center.y)
        };
        if let Some(f) = t.draw_stop_indicator {
            f(d, stop_indicator_offset);
        }
    }
    // active track
    let active_track_start = if t.is_centered {
        adjusted_slider_value_end + if adjusted_slider_value_end < center_axis { start_gap } else { 0.0 }
    } else if t.is_range_slider {
        slider_value_start + start_gap
    } else {
        0.0
    };
    let active_track_end = if t.is_centered {
        adjusted_slider_value_start - if adjusted_slider_value_start > center_axis { end_gap } else { 0.0 }
    } else {
        slider_value_end - end_gap
    };
    let start_corner_radius =
        if is_rtl_horizontal || t.is_centered || t.is_range_slider { inside_corner_size } else { corner_size };
    let end_corner_radius =
        if is_rtl_horizontal && !t.is_centered && !t.is_range_slider { corner_size } else { inside_corner_size };
    let active_track_width = if is_rtl_horizontal && !t.is_centered && !t.is_range_slider {
        active_track_end
    } else {
        active_track_end - active_track_start
    };

    let active_track_threshold =
        if !t.enable_corner_shrinking || !tick_fractions.is_empty() { start_corner_radius } else { 0.0 };
    if active_track_width > active_track_threshold {
        let track_offset = if is_vertical {
            Offset::new(0.0, active_track_start)
        } else if is_rtl {
            Offset::new(size.width - active_track_end, 0.0)
        } else {
            Offset::new(active_track_start, 0.0)
        };

        let track_size = if is_vertical {
            Size::new(size.width, active_track_width)
        } else if is_rtl && !t.is_centered && !t.is_range_slider {
            Size::new(active_track_end, size.height)
        } else {
            Size::new(active_track_width, size.height)
        };
        draw_track_path(
            d,
            t.orientation,
            track_offset,
            track_size,
            t.active_track_color,
            start_corner_radius,
            end_corner_radius,
        );
    }

    let start = slider_start + corner_size;
    let end = slider_end - corner_size;
    let active_track = (active_track_start, active_track_end);
    let tick_center_gap = (center_axis - end_gap, center_axis + end_gap);
    let tick_start_gap = (slider_value_start - start_gap, slider_value_start + start_gap);
    let tick_end_gap = (slider_value_end - end_gap, slider_value_end + end_gap);
    for (index, &tick) in tick_fractions.iter().enumerate() {
        // skip ticks that fall on the stop indicator
        if t.draw_stop_indicator.is_some() {
            let is_stop_indicator_at_start = (t.is_centered || t.is_range_slider) && index == 0;
            if is_stop_indicator_at_start || index == tick_fractions.len() - 1 {
                continue;
            }
        }

        let center_tick = lerp(start, end, tick);
        // skip ticks that fall on a gap
        if (t.is_centered && contains(tick_center_gap.0, tick_center_gap.1, center_tick))
            || (t.is_range_slider && contains(tick_start_gap.0, tick_start_gap.1, center_tick))
            || contains(tick_end_gap.0, tick_end_gap.1, center_tick)
        {
            continue;
        }
        let offset = if is_vertical {
            Offset::new(center.x, center_tick)
        } else if is_rtl {
            Offset::new(size.width - center_tick, center.y)
        } else {
            Offset::new(center_tick, center.y)
        };
        let color = if contains(active_track.0, active_track.1, center_tick) {
            t.active_tick_color
        } else {
            t.inactive_tick_color
        };
        (t.draw_tick)(d, offset, color);
    }
}

thread_local! {
    /// The path the stretches are drawn with (Compose's `SliderDefaults.trackPath`, one path rewound after each).
    static TRACK_PATH: RefCell<Path> = RefCell::new(Path::new());
}

/// One stretch: a rect with `start_corner_radius` corners at its start, `end_corner_radius` at its end (Compose's
/// private `DrawScope.drawTrackPath`).
fn draw_track_path(
    d: &mut DrawScope,
    orientation: Orientation,
    offset: Offset,
    size: Size,
    color: Color,
    start_corner_radius: f32,
    end_corner_radius: f32,
) {
    let start_corner = CornerRadius::new(start_corner_radius, start_corner_radius);
    let end_corner = CornerRadius::new(end_corner_radius, end_corner_radius);
    let rect = Rect::from_offset_size(offset, Size::new(size.width, size.height));
    let track = if orientation == Orientation::Vertical {
        RoundRect::from_rect(rect, start_corner, start_corner, end_corner, end_corner)
    } else {
        RoundRect::from_rect(rect, start_corner, end_corner, end_corner, start_corner)
    };
    TRACK_PATH.with(|path| {
        let mut path = path.borrow_mut();
        path.add_round_rect(&track, PathDirection::CounterClockwise);
        d.draw_path(&path, color);
        path.rewind();
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The thumb

/// The thumb (Compose's private `Thumb`): a bar of `thumb_size`, half as thick while its source says it is
/// pressed, focused or dragged (Compose's list of those interactions, by their identities); hoverable, a hand over
/// it.
#[kompozable]
fn Thumb(
    interaction_source: MutableInteractionSource,
    modifier: Modifier,
    colors: SliderColors,
    enabled: bool,
    thumb_size: DpSize,
    is_vertical: bool,
) {
    let interactions = remember(|| mutable_state_list_of::<InteractionId>(Vec::new()));
    let (list, source) = (interactions.clone(), interaction_source.clone());
    LaunchedEffect(
        interaction_source.clone(),
        source.interactions().collect(move |interaction: Arc<dyn Interaction>| {
            let remove = |id: InteractionId| {
                list.update(|v| {
                    if let Some(i) = v.iter().position(|x| *x == id) {
                        v.remove(i);
                    }
                })
            };
            if let Some(focus) = interaction.downcast_ref::<FocusInteraction::Focus>() {
                list.add(focus.id());
            } else if let Some(unfocus) = interaction.downcast_ref::<FocusInteraction::Unfocus>() {
                remove(unfocus.focus.id());
            } else if let Some(press) = interaction.downcast_ref::<PressInteraction::Press>() {
                list.add(press.id());
            } else if let Some(release) = interaction.downcast_ref::<PressInteraction::Release>() {
                remove(release.press.id());
            } else if let Some(cancel) = interaction.downcast_ref::<PressInteraction::Cancel>() {
                remove(cancel.press.id());
            } else if let Some(start) = interaction.downcast_ref::<DragInteraction::Start>() {
                list.add(start.id());
            } else if let Some(stop) = interaction.downcast_ref::<DragInteraction::Stop>() {
                remove(stop.start.id());
            } else if let Some(cancel) = interaction.downcast_ref::<DragInteraction::Cancel>() {
                remove(cancel.start.id());
            }
        }),
    );

    let size = if !interactions.is_empty() {
        if is_vertical {
            DpSize { width: thumb_size.width, height: thumb_size.height / 2.0 }
        } else {
            DpSize { width: thumb_size.width / 2.0, height: thumb_size.height }
        }
    } else {
        thumb_size
    };
    Spacer(
        modifier
            .size_dp(size)
            .hoverable(interaction_source)
            .pointer_hover_icon(PointerIcon::Hand)
            .background(colors.thumb_color_of(enabled), SliderTokens::HandleShape.value()),
    );
}

// ---------------------------------------------------------------------------------------------------------------
// SliderColors

/// The colors of a slider in its states (Compose's SliderColors): the thumb's, the active and the inactive
/// stretches' and ticks', enabled or not. Make them with [`SliderDefaults::colors`] or
/// [`SliderColors()`](fn@SliderColors); change some with [`copy`](SliderColors::copy).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, KotlinProperties)]
pub struct SliderColors {
    pub thumb_color: Color,
    pub active_track_color: Color,
    pub active_tick_color: Color,
    pub inactive_track_color: Color,
    pub inactive_tick_color: Color,
    pub disabled_thumb_color: Color,
    pub disabled_active_track_color: Color,
    pub disabled_active_tick_color: Color,
    pub disabled_inactive_track_color: Color,
    pub disabled_inactive_tick_color: Color,
}

/// A slider's colors, each given (Compose's `SliderColors(…)` constructor).
#[named(type)]
#[allow(clippy::too_many_arguments)]
pub fn SliderColors(
    thumb_color: Color,
    active_track_color: Color,
    active_tick_color: Color,
    inactive_track_color: Color,
    inactive_tick_color: Color,
    disabled_thumb_color: Color,
    disabled_active_track_color: Color,
    disabled_active_tick_color: Color,
    disabled_inactive_track_color: Color,
    disabled_inactive_tick_color: Color,
) -> SliderColors {
    SliderColors {
        thumb_color,
        active_track_color,
        active_tick_color,
        inactive_track_color,
        inactive_tick_color,
        disabled_thumb_color,
        disabled_active_track_color,
        disabled_active_tick_color,
        disabled_inactive_track_color,
        disabled_inactive_tick_color,
    }
}

#[named]
impl SliderColors {
    /// The same with what is given replaced (Compose's `copy`): an unspecified color keeps this one's.
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.thumb_color)] thumb_color: Color,
        #[default(self.active_track_color)] active_track_color: Color,
        #[default(self.active_tick_color)] active_tick_color: Color,
        #[default(self.inactive_track_color)] inactive_track_color: Color,
        #[default(self.inactive_tick_color)] inactive_tick_color: Color,
        #[default(self.disabled_thumb_color)] disabled_thumb_color: Color,
        #[default(self.disabled_active_track_color)] disabled_active_track_color: Color,
        #[default(self.disabled_active_tick_color)] disabled_active_tick_color: Color,
        #[default(self.disabled_inactive_track_color)] disabled_inactive_track_color: Color,
        #[default(self.disabled_inactive_tick_color)] disabled_inactive_tick_color: Color,
    ) -> SliderColors {
        SliderColors(
            thumb_color.take_or_else(|| self.thumb_color),
            active_track_color.take_or_else(|| self.active_track_color),
            active_tick_color.take_or_else(|| self.active_tick_color),
            inactive_track_color.take_or_else(|| self.inactive_track_color),
            inactive_tick_color.take_or_else(|| self.inactive_tick_color),
            disabled_thumb_color.take_or_else(|| self.disabled_thumb_color),
            disabled_active_track_color.take_or_else(|| self.disabled_active_track_color),
            disabled_active_tick_color.take_or_else(|| self.disabled_active_tick_color),
            disabled_inactive_track_color.take_or_else(|| self.disabled_inactive_track_color),
            disabled_inactive_tick_color.take_or_else(|| self.disabled_inactive_tick_color),
        )
    }
}

impl SliderColors {
    /// The thumb's color (Compose's internal `thumbColor(enabled)`; named apart from the `thumb_color` property's
    /// getter here).
    pub(crate) fn thumb_color_of(&self, enabled: bool) -> Color {
        if enabled { self.thumb_color } else { self.disabled_thumb_color }
    }

    /// A stretch's color (Compose's internal `trackColor`).
    pub(crate) fn track_color(&self, enabled: bool, active: bool) -> Color {
        if enabled {
            if active { self.active_track_color } else { self.inactive_track_color }
        } else if active {
            self.disabled_active_track_color
        } else {
            self.disabled_inactive_track_color
        }
    }

    /// A tick's color (Compose's internal `tickColor`).
    pub(crate) fn tick_color(&self, enabled: bool, active: bool) -> Color {
        if enabled {
            if active { self.active_tick_color } else { self.inactive_tick_color }
        } else if active {
            self.disabled_active_tick_color
        } else {
            self.disabled_inactive_tick_color
        }
    }
}
