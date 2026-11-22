//! Material's progress indicators, as Compose's ProgressIndicator.kt (material3 1.5): a bar
//! ([`LinearProgressIndicator`]) and a ring ([`CircularProgressIndicator`]) filled to a progress read when they
//! draw, their endless forms ([`IndeterminateLinearProgressIndicator`], [`IndeterminateCircularProgressIndicator`])
//! and the deprecated forms of a progress given as a value ([`FloatLinearProgressIndicator`],
//! [`FloatCircularProgressIndicator`]); [`ProgressIndicatorDefaults`].
//!
//! The progress keeps a gap from its track (4 dp, and the round caps' room), the bar ends in a stop indicator. What
//! they tell accessibility is a progress in 0..1 (an endless one: an indeterminate one).

// (the deprecated forms' own generated items name them; the deprecated circularTrackColor is their default)
#![allow(deprecated)]

use crate::internal::accessibility_util::IncreaseVerticalSemanticsBounds;
use crate::tokens::{
    CircularProgressIndicatorTokens, LinearProgressIndicatorTokens, MotionTokens, ProgressIndicatorTokens,
};
use kompoz_animation::{
    InfiniteRepeatableSpec, KeyframesSpec, LinearEasing, RepeatMode, Spring, SpringSpec, StartOffset, TweenSpec,
    keyframes, remember_infinite_transition, tween,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, DrawStyle, LayoutDirection, Offset, Size, Stroke, StrokeCap, StrokeJoin};
use kompoz_runtime::{kompozable, named_args};
use kompoz_ui::Modifier;
use kompoz_ui::draw::DrawScope;
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::semantics::ProgressBarRangeInfo;
use std::f64::consts::PI;
use std::rc::Rc;
use std::sync::Arc;

/// What draws a linear indicator's stop (Compose's `drawStopIndicator: DrawScope.() -> Unit`).
pub type DrawStopIndicatorLambda = Rc<dyn Fn(&mut DrawScope)>;

// ---- LinearProgressIndicator ------------------------------------------------------------------------------------

/// A determinate linear progress indicator (Compose's `LinearProgressIndicator(progress: () -> Float, …, gapSize,
/// drawStopIndicator)`): a bar `progress()` of the way along in `color` (the progress read as it draws, kept in
/// 0..1; NaN told to accessibility as 0), the rest of its `track_color` track `gap_size` (and the caps' room) apart,
/// ends `stroke_cap`, then `draw_stop_indicator` (None: Kotlin's default, [`ProgressIndicatorDefaults::
/// draw_stop_indicator`] of [`ProgressIndicatorDefaults::LinearTrackStopIndicatorSize`] in `color` and `stroke_cap`).
/// 240 by 4 dp unless the modifier sizes it; its semantics bounds 4 dp taller at each side.
///
/// Kotlin: `LinearProgressIndicator(progress: () -> Float, gapSize: Dp, drawStopIndicator: DrawScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn LinearProgressIndicator(
    progress: impl Fn() -> f32 + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(ProgressIndicatorDefaults::linear_color())] color: Color,
    #[default(ProgressIndicatorDefaults::linear_track_color())] track_color: Color,
    #[default(ProgressIndicatorDefaults::LinearStrokeCap)] stroke_cap: StrokeCap,
    #[default(ProgressIndicatorDefaults::LinearIndicatorTrackGapSize)] gap_size: Dp,
    #[default(None)] draw_stop_indicator: Option<DrawStopIndicatorLambda>,
) {
    let draw_stop_indicator = draw_stop_indicator.unwrap_or_else(|| {
        Rc::new(move |d: &mut DrawScope| {
            ProgressIndicatorDefaults.draw_stop_indicator(
                d,
                ProgressIndicatorDefaults::LinearTrackStopIndicatorSize,
                color,
                stroke_cap,
            )
        })
    });
    let progress: Rc<dyn Fn() -> f32> = Rc::new(progress);
    let coerced_progress = move || fast_coerce_in(progress(), 0.0, 1.0);
    let semantics_progress = coerced_progress.clone();
    Canvas(
        modifier
            .then(IncreaseVerticalSemanticsBounds())
            .semantics_with(true, move |c| {
                // Check for NaN, as the ProgressBarRangeInfo will throw an exception.
                let current = Some(semantics_progress()).filter(|p| !p.is_nan()).unwrap_or(0.0);
                c.set_progress_bar_range_info(ProgressBarRangeInfo::new(current, 0.0..=1.0, 0));
            })
            .size_wh(LinearIndicatorWidth, LinearIndicatorHeight),
        move |d| {
            let size = d.size();
            let stroke_width = size.height;
            let adjusted_gap_size = if stroke_cap == StrokeCap::Butt || size.height > size.width {
                gap_size
            } else {
                gap_size + d.to_dp(stroke_width)
            };
            let gap_size_fraction = adjusted_gap_size.0 / d.to_dp(size.width).0;
            let current_coerced_progress = coerced_progress();

            // track
            let track_start_fraction = current_coerced_progress + current_coerced_progress.min(gap_size_fraction);
            if track_start_fraction <= 1.0 {
                draw_linear_indicator(d, track_start_fraction, 1.0, track_color, stroke_width, stroke_cap);
            }
            // indicator
            draw_linear_indicator(d, 0.0, current_coerced_progress, color, stroke_width, stroke_cap);
            // stop
            draw_stop_indicator(d);
        },
    );
}

/// An indeterminate linear progress indicator (Compose's `LinearProgressIndicator(modifier, color, trackColor,
/// strokeCap, gapSize)`): two lines in `color` chasing along a `track_color` track forever (1.75 s a round), the
/// track `gap_size` (and the caps' room) from each. It tells accessibility an indeterminate progress.
///
/// Kotlin: `LinearProgressIndicator(modifier: Modifier, gapSize: Dp)`
#[kompozable]
pub fn IndeterminateLinearProgressIndicator(
    #[default(Modifier)] modifier: Modifier,
    #[default(ProgressIndicatorDefaults::linear_color())] color: Color,
    #[default(ProgressIndicatorDefaults::linear_track_color())] track_color: Color,
    #[default(ProgressIndicatorDefaults::LinearStrokeCap)] stroke_cap: StrokeCap,
    #[default(ProgressIndicatorDefaults::LinearIndicatorTrackGapSize)] gap_size: Dp,
) {
    let infinite_transition = remember_infinite_transition();
    let first_line_head =
        infinite_transition.animate_float(0.0, 1.0, linear_indeterminate_first_line_head_animation_spec());
    let first_line_tail =
        infinite_transition.animate_float(0.0, 1.0, linear_indeterminate_first_line_tail_animation_spec());
    let second_line_head =
        infinite_transition.animate_float(0.0, 1.0, linear_indeterminate_second_line_head_animation_spec());
    let second_line_tail =
        infinite_transition.animate_float(0.0, 1.0, linear_indeterminate_second_line_tail_animation_spec());
    Canvas(
        modifier
            .then(IncreaseVerticalSemanticsBounds())
            .progress_semantics_indeterminate()
            .size_wh(LinearIndicatorWidth, LinearIndicatorHeight),
        move |d| {
            let size = d.size();
            let stroke_width = size.height;
            let adjusted_gap_size = if stroke_cap == StrokeCap::Butt || size.height > size.width {
                gap_size
            } else {
                gap_size + d.to_dp(stroke_width)
            };
            let gap_size_fraction = adjusted_gap_size.0 / d.to_dp(size.width).0;
            let (first_head, first_tail) = (first_line_head.value(), first_line_tail.value());
            let (second_head, second_tail) = (second_line_head.value(), second_line_tail.value());

            // Track before line 1
            if first_head < 1.0 - gap_size_fraction {
                let start = if first_head > 0.0 { first_head + gap_size_fraction } else { 0.0 };
                draw_linear_indicator(d, start, 1.0, track_color, stroke_width, stroke_cap);
            }

            // Line 1
            if first_head - first_tail > 0.0 {
                draw_linear_indicator(d, first_head, first_tail, color, stroke_width, stroke_cap);
            }

            // Track between line 1 and line 2
            if first_tail > gap_size_fraction {
                let start = if second_head > 0.0 { second_head + gap_size_fraction } else { 0.0 };
                let end = if first_tail < 1.0 { first_tail - gap_size_fraction } else { 1.0 };
                draw_linear_indicator(d, start, end, track_color, stroke_width, stroke_cap);
            }

            // Line 2
            if second_head - second_tail > 0.0 {
                draw_linear_indicator(d, second_head, second_tail, color, stroke_width, stroke_cap);
            }

            // Track after line 2
            if second_tail > gap_size_fraction {
                let end = if second_tail < 1.0 { second_tail - gap_size_fraction } else { 1.0 };
                draw_linear_indicator(d, 0.0, end, track_color, stroke_width, stroke_cap);
            }
        },
    );
}

/// A determinate linear progress indicator of a progress given as a value (Compose's deprecated
/// `LinearProgressIndicator(progress: Float, …)`): [`LinearProgressIndicator`] of `{ progress }`, its gap and stop
/// indicator the defaults.
///
/// Kotlin: `LinearProgressIndicator(progress: Float)`
#[deprecated(note = "Use the overload that takes `progress` as a lambda")]
#[kompozable]
pub fn FloatLinearProgressIndicator(
    progress: f32,
    #[default(Modifier)] modifier: Modifier,
    #[default(ProgressIndicatorDefaults::linear_color())] color: Color,
    #[default(ProgressIndicatorDefaults::linear_track_color())] track_color: Color,
    #[default(ProgressIndicatorDefaults::LinearStrokeCap)] stroke_cap: StrokeCap,
) {
    LinearProgressIndicator(
        progress = move || progress,
        modifier = modifier,
        color = color,
        track_color = track_color,
        stroke_cap = stroke_cap,
    );
}

/// Compose's private `DrawScope.drawLinearIndicator`: a line from `start_fraction` to `end_fraction` of the width
/// (from the right, right to left) along the middle, `stroke_width` thick; its caps `stroke_cap` kept inside unless
/// there is no room for them (taller than wide), then butt.
fn draw_linear_indicator(
    d: &mut DrawScope,
    start_fraction: f32,
    end_fraction: f32,
    color: Color,
    stroke_width: f32,
    stroke_cap: StrokeCap,
) {
    let size = d.size();
    let (width, height) = (size.width, size.height);
    // Start drawing from the vertical center of the stroke
    let y_offset = height / 2.0;

    let is_ltr = d.layout_direction() == LayoutDirection::Ltr;
    let bar_start = (if is_ltr { start_fraction } else { 1.0 - end_fraction }) * width;
    let bar_end = (if is_ltr { end_fraction } else { 1.0 - start_fraction }) * width;

    // if there isn't enough space to draw the stroke caps, fall back to StrokeCap.Butt
    if stroke_cap == StrokeCap::Butt || height > width {
        // Progress line
        named_args!(d.draw_line(
            color,
            Offset::new(bar_start, y_offset),
            Offset::new(bar_end, y_offset),
            stroke_width = stroke_width
        ));
    } else {
        // need to adjust barStart and barEnd for the stroke caps
        let stroke_cap_offset = stroke_width / 2.0;
        let adjusted_bar_start = fast_coerce_in(bar_start, stroke_cap_offset, width - stroke_cap_offset);
        let adjusted_bar_end = fast_coerce_in(bar_end, stroke_cap_offset, width - stroke_cap_offset);

        if (end_fraction - start_fraction).abs() > 0.0 {
            // Progress line
            named_args!(d.draw_line(
                color,
                Offset::new(adjusted_bar_start, y_offset),
                Offset::new(adjusted_bar_end, y_offset),
                stroke_width = stroke_width,
                cap = stroke_cap
            ));
        }
    }
}

/// Compose's `fastCoerceIn`: `min` below it, `max` above it (in that order: no check that `min <= max`), NaN as it
/// is.
pub(crate) fn fast_coerce_in(value: f32, min: f32, max: f32) -> f32 {
    let mut v = value;
    if v < min {
        v = min;
    }
    if v > max {
        v = max;
    }
    v
}

/// Compose's `fastCoerceAtMost`: `max` above it, NaN as it is.
pub(crate) fn fast_coerce_at_most(value: f32, max: f32) -> f32 {
    if value > max { max } else { value }
}

// ---- CircularProgressIndicator ----------------------------------------------------------------------------------

/// A determinate circular progress indicator (Compose's `CircularProgressIndicator(progress: () -> Float, …,
/// gapSize)`): from 12 o'clock clockwise, `progress()` of the ring in `color` (read as it draws, kept in 0..1), the
/// rest of the ring in `track_color` `gap_size` (and the caps' room) from both its ends; `stroke_width` thick, ends
/// `stroke_cap`. 40 dp across unless the modifier sizes it.
///
/// Kotlin: `CircularProgressIndicator(progress: () -> Float, gapSize: Dp)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn CircularProgressIndicator(
    progress: impl Fn() -> f32 + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(ProgressIndicatorDefaults::circular_color())] color: Color,
    #[default(ProgressIndicatorDefaults::CircularStrokeWidth)] stroke_width: Dp,
    #[default(ProgressIndicatorDefaults::circular_determinate_track_color())] track_color: Color,
    #[default(ProgressIndicatorDefaults::CircularDeterminateStrokeCap)] stroke_cap: StrokeCap,
    #[default(ProgressIndicatorDefaults::CircularIndicatorTrackGapSize)] gap_size: Dp,
) {
    let progress: Rc<dyn Fn() -> f32> = Rc::new(progress);
    let coerced_progress = move || fast_coerce_in(progress(), 0.0, 1.0);
    let semantics_progress = coerced_progress.clone();
    let stroke = circular_stroke(LocalDensity.current().to_px(stroke_width), stroke_cap);
    Canvas(
        modifier
            .semantics_with(true, move |c| {
                // Check for NaN, as the ProgressBarRangeInfo will throw an exception.
                let current = Some(semantics_progress()).filter(|p| !p.is_nan()).unwrap_or(0.0);
                c.set_progress_bar_range_info(ProgressBarRangeInfo::new(current, 0.0..=1.0, 0));
            })
            .size(CircularIndicatorDiameter),
        move |d| {
            let size = d.size();
            // Start at 12 o'clock
            let start_angle = 270.0f32;
            let sweep = coerced_progress() * 360.0;
            let adjusted_gap_size = if stroke_cap == StrokeCap::Butt || size.height > size.width {
                gap_size
            } else {
                gap_size + stroke_width
            };
            let gap_size_sweep = (adjusted_gap_size.0 / (PI * d.to_dp(size.width).0 as f64) as f32) * 360.0;

            draw_circular_indicator(
                d,
                start_angle + sweep + sweep.min(gap_size_sweep),
                360.0 - sweep - sweep.min(gap_size_sweep) * 2.0,
                track_color,
                &stroke,
            );
            draw_determinate_circular_indicator(d, start_angle, sweep, color, &stroke);
        },
    );
}

/// An indeterminate circular progress indicator (Compose's `CircularProgressIndicator(modifier, color,
/// strokeWidth, trackColor, strokeCap, gapSize)`): an arc in `color` growing from a tenth of the ring to 87% and
/// back each 6 s, turning 1080° in that time and a quarter more in 300 ms every 1.5 s; its `track_color` track (none
/// by default: transparent) the rest of the ring `gap_size` (and the caps' room) from its ends. It tells
/// accessibility an indeterminate progress.
///
/// Kotlin: `CircularProgressIndicator(modifier: Modifier, gapSize: Dp)`
#[kompozable]
pub fn IndeterminateCircularProgressIndicator(
    #[default(Modifier)] modifier: Modifier,
    #[default(ProgressIndicatorDefaults::circular_color())] color: Color,
    #[default(ProgressIndicatorDefaults::CircularStrokeWidth)] stroke_width: Dp,
    #[default(ProgressIndicatorDefaults::circular_indeterminate_track_color())] track_color: Color,
    #[default(ProgressIndicatorDefaults::CircularIndeterminateStrokeCap)] stroke_cap: StrokeCap,
    #[default(ProgressIndicatorDefaults::CircularIndicatorTrackGapSize)] gap_size: Dp,
) {
    let stroke = circular_stroke(LocalDensity.current().to_px(stroke_width), stroke_cap);

    let infinite_transition = remember_infinite_transition();
    // A global rotation that does a 360 degrees rotation in 6 seconds.
    let global_rotation = infinite_transition.animate_float(
        0.0,
        CircularGlobalRotationDegreesTarget,
        circular_indeterminate_global_rotation_animation_spec(),
    );

    // An additional rotation that moves by 90 degrees in 500ms and then rest for 1 second.
    let additional_rotation = infinite_transition.animate_float(
        0.0,
        CircularAdditionalRotationDegreesTarget,
        circular_indeterminate_rotation_animation_spec(),
    );

    // Indicator progress animation that will be changing the progress up and down as the indicator
    // rotates.
    let progress_animation = infinite_transition.animate_float(
        CircularIndeterminateMinProgress,
        CircularIndeterminateMaxProgress,
        circular_indeterminate_progress_animation_spec(),
    );

    Canvas(modifier.progress_semantics_indeterminate().size(CircularIndicatorDiameter), move |d| {
        let size = d.size();
        let sweep = progress_animation.value() * 360.0;
        let adjusted_gap_size =
            if stroke_cap == StrokeCap::Butt || size.height > size.width { gap_size } else { gap_size + stroke_width };
        let gap_size_sweep = (adjusted_gap_size.0 / (PI * d.to_dp(size.width).0 as f64) as f32) * 360.0;

        let rotation = global_rotation.value() + additional_rotation.value();
        let pivot = d.center();
        d.rotate_with(rotation, pivot, |d| {
            draw_circular_indicator(
                d,
                sweep + sweep.min(gap_size_sweep),
                360.0 - sweep - sweep.min(gap_size_sweep) * 2.0,
                track_color,
                &stroke,
            );
            draw_determinate_circular_indicator(d, 0.0, sweep, color, &stroke);
        });
    });
}

/// A determinate circular progress indicator of a progress given as a value (Compose's deprecated
/// `CircularProgressIndicator(progress: Float, …)`): [`CircularProgressIndicator`] of `{ progress }`, its track
/// transparent by default (the deprecated [`ProgressIndicatorDefaults::circular_track_color`]), its gap the
/// default.
///
/// Kotlin: `CircularProgressIndicator(progress: Float)`
#[deprecated(note = "Use the overload that takes `progress` as a lambda")]
#[kompozable]
pub fn FloatCircularProgressIndicator(
    progress: f32,
    #[default(Modifier)] modifier: Modifier,
    #[default(ProgressIndicatorDefaults::circular_color())] color: Color,
    #[default(ProgressIndicatorDefaults::CircularStrokeWidth)] stroke_width: Dp,
    #[default(ProgressIndicatorDefaults::circular_track_color())] track_color: Color,
    #[default(ProgressIndicatorDefaults::CircularDeterminateStrokeCap)] stroke_cap: StrokeCap,
) {
    CircularProgressIndicator(
        progress = move || progress,
        modifier = modifier,
        color = color,
        stroke_width = stroke_width,
        track_color = track_color,
        stroke_cap = stroke_cap,
    );
}

/// Compose's `Stroke(width = strokeWidth.toPx(), cap = strokeCap)`.
fn circular_stroke(width: f32, cap: StrokeCap) -> Stroke {
    Stroke::new(width, Stroke::DefaultMiter, cap, StrokeJoin::Miter, None)
}

/// Compose's private `DrawScope.drawCircularIndicator`: an arc from `start_angle` through `sweep` of the circle
/// whose edges line up with the stroke's middle.
fn draw_circular_indicator(d: &mut DrawScope, start_angle: f32, sweep: f32, color: Color, stroke: &Stroke) {
    // To draw this circle we need a rect with edges that line up with the midpoint of the stroke.
    // To do this we need to remove half the stroke width from the total diameter for both sides.
    let diameter_offset = stroke.width / 2.0;
    let arc_dimen = d.size().width - 2.0 * diameter_offset;
    named_args!(d.draw_arc(
        color,
        start_angle,
        sweep,
        false,
        top_left = Offset::new(diameter_offset, diameter_offset),
        size = Size::new(arc_dimen, arc_dimen),
        style = DrawStyle::Stroke(stroke.clone())
    ));
}

/// Compose's private `DrawScope.drawCircularIndicatorTrack`: the whole ring.
#[allow(dead_code)]
fn draw_circular_indicator_track(d: &mut DrawScope, color: Color, stroke: &Stroke) {
    draw_circular_indicator(d, 0.0, 360.0, color, stroke)
}

/// Compose's private `DrawScope.drawDeterminateCircularIndicator`.
fn draw_determinate_circular_indicator(d: &mut DrawScope, start_angle: f32, sweep: f32, color: Color, stroke: &Stroke) {
    draw_circular_indicator(d, start_angle, sweep, color, stroke)
}

/// Compose's private `DrawScope.drawIndeterminateCircularIndicator` (no longer called there): the start moved on
/// by the cap's room, at least a 0.1° sweep.
#[allow(dead_code)]
fn draw_indeterminate_circular_indicator(
    d: &mut DrawScope,
    start_angle: f32,
    stroke_width: Dp,
    sweep: f32,
    color: Color,
    stroke: &Stroke,
) {
    let stroke_cap_offset = if stroke.cap == StrokeCap::Butt {
        0.0
    } else {
        // Length of arc is angle * radius
        // Angle (radians) is length / radius
        // The length should be the same as the stroke width for calculating the min angle
        (180.0 / PI) as f32 * (stroke_width.0 / (CircularIndicatorDiameter.0 / 2.0)) / 2.0
    };

    // Adding a stroke cap draws half the stroke width behind the start point, so we want to
    // move it forward by that amount so the arc visually appears in the correct place
    let adjusted_start_angle = start_angle + stroke_cap_offset;

    // When the start and end angles are in the same place, we still want to draw a small sweep, so
    // the stroke caps get added on both ends and we draw the correct minimum length arc
    let adjusted_sweep = sweep.max(0.1);

    draw_circular_indicator(d, adjusted_start_angle, adjusted_sweep, color, stroke);
}

// ---- ProgressIndicatorDefaults ----------------------------------------------------------------------------------

/// The progress indicators' defaults (Compose's `ProgressIndicatorDefaults`): used as a value
/// (`ProgressIndicatorDefaults.draw_stop_indicator(…)`), its properties associated items.
pub struct ProgressIndicatorDefaults;

#[allow(non_snake_case)]
impl ProgressIndicatorDefaults {
    /// A linear indicator's color: the scheme's primary (Compose's `linearColor`).
    pub fn linear_color() -> Color {
        ProgressIndicatorTokens::ActiveIndicatorColor.value()
    }

    /// A circular indicator's color: the scheme's primary (Compose's `circularColor`).
    pub fn circular_color() -> Color {
        ProgressIndicatorTokens::ActiveIndicatorColor.value()
    }

    /// A linear indicator's track: the scheme's secondary container (Compose's `linearTrackColor`).
    pub fn linear_track_color() -> Color {
        ProgressIndicatorTokens::TrackColor.value()
    }

    /// A circular indicator's track: transparent (Compose's deprecated `circularTrackColor`).
    #[deprecated(note = "Renamed to circularDeterminateTrackColor or circularIndeterminateTrackColor")]
    pub fn circular_track_color() -> Color {
        Color::Transparent
    }

    /// A determinate circular indicator's track: the scheme's secondary container (Compose's
    /// `circularDeterminateTrackColor`).
    pub fn circular_determinate_track_color() -> Color {
        ProgressIndicatorTokens::TrackColor.value()
    }

    /// An indeterminate circular indicator's track: transparent (Compose's `circularIndeterminateTrackColor`).
    pub fn circular_indeterminate_track_color() -> Color {
        Color::Transparent
    }

    /// A circular indicator's stroke width: 4 dp.
    pub const CircularStrokeWidth: Dp = CircularProgressIndicatorTokens::TrackThickness;

    /// A linear indicator's caps: round.
    pub const LinearStrokeCap: StrokeCap = StrokeCap::Round;

    /// A determinate circular indicator's caps: round.
    pub const CircularDeterminateStrokeCap: StrokeCap = StrokeCap::Round;

    /// An indeterminate circular indicator's caps: round.
    pub const CircularIndeterminateStrokeCap: StrokeCap = StrokeCap::Round;

    /// A linear indicator's stop indicator size: 4 dp.
    pub const LinearTrackStopIndicatorSize: Dp = LinearProgressIndicatorTokens::StopSize;

    /// A linear indicator's gap between its progress and its track: 4 dp.
    pub const LinearIndicatorTrackGapSize: Dp = LinearProgressIndicatorTokens::TrackActiveSpace;

    /// A circular indicator's gap between its progress and its track: 4 dp.
    pub const CircularIndicatorTrackGapSize: Dp = CircularProgressIndicatorTokens::TrackActiveSpace;

    /// The spec to animate a progress with (Compose's `ProgressAnimationSpec`): a spring of no bounce and very low
    /// stiffness, at rest within a thousandth.
    pub fn ProgressAnimationSpec() -> SpringSpec<f32> {
        SpringSpec::new(
            Spring::DampingRatioNoBouncy,
            Spring::StiffnessVeryLow,
            // The default threshold is 0.01, or 1% of the overall progress range, which is quite
            // large and noticeable. We purposefully choose a smaller threshold.
            Some(1.0 / 1000.0),
        )
    }

    /// Draws a linear indicator's stop: a dot (a square unless `stroke_cap` is round) of `stop_size` (no taller
    /// than the track) in `color` at its end — at most 6 dp from it, as far as it is from the top — mirrored right to
    /// left (Compose's `drawStopIndicator(drawScope, stopSize, color, strokeCap)`).
    ///
    /// Kotlin: `drawStopIndicator(drawScope: DrawScope, stopSize: Dp, color: Color, strokeCap: StrokeCap)`
    pub fn draw_stop_indicator(&self, draw_scope: &mut DrawScope, stop_size: Dp, color: Color, stroke_cap: StrokeCap) {
        // Draws a circle or a square indicator according to the provided Cap.
        let draw_indicator = |d: &mut DrawScope, adjusted_stop_size: f32, stop_offset: f32| {
            let size = d.size();
            if stroke_cap == StrokeCap::Round {
                named_args!(d.draw_circle(
                    color,
                    radius = adjusted_stop_size / 2.0,
                    center = Offset::new(size.width - (adjusted_stop_size / 2.0) - stop_offset, size.height / 2.0)
                ));
            } else {
                named_args!(d.draw_rect(
                    color,
                    top_left = Offset::new(
                        size.width - adjusted_stop_size - stop_offset,
                        (size.height - adjusted_stop_size) / 2.0
                    ),
                    size = Size::new(adjusted_stop_size, adjusted_stop_size)
                ));
            }
        };

        let d = draw_scope;
        let size = d.size();
        let adjusted_stop_size = d.to_px(stop_size).min(size.height); // Stop can't be bigger than track
        // The limit to prevent excessive padding when dealing with large progress height.
        // TODO b/401511176 - Consider adding a new token.
        let max_stop_offset = d.to_px(StopIndicatorTrailingSpace);
        // Offset from end.
        let stop_offset = fast_coerce_at_most((size.height - adjusted_stop_size) / 2.0, max_stop_offset);
        if d.layout_direction() == LayoutDirection::Rtl {
            // Flip the direction by scaling with -1 on the X axis.
            let pivot = d.center();
            d.scale_with(-1.0, 1.0, pivot, |d| draw_indicator(d, adjusted_stop_size, stop_offset));
        } else {
            draw_indicator(d, adjusted_stop_size, stop_offset);
        }
    }
}

// ---- The indeterminate animations' specs and the sizes ----------------------------------------------------------

/// Compose's internal `circularIndeterminateGlobalRotationAnimationSpec`: linear, 6 s a round.
pub(crate) fn circular_indeterminate_global_rotation_animation_spec() -> InfiniteRepeatableSpec<f32> {
    let animation: TweenSpec<f32> =
        named_args!(tween(duration_millis = CircularAnimationProgressDuration, easing = Arc::new(LinearEasing)));
    InfiniteRepeatableSpec::new(animation, RepeatMode::Restart, StartOffset::default())
}

/// Compose's internal `circularIndeterminateRotationAnimationSpec`: a quarter more in 300 ms every 1.5 s.
pub(crate) fn circular_indeterminate_rotation_animation_spec() -> InfiniteRepeatableSpec<f32> {
    let animation: KeyframesSpec<f32> = keyframes(|k| {
        k.duration_millis = CircularAnimationProgressDuration; // 6000ms
        k.at(90.0, CircularAnimationAdditionalRotationDuration)
            .using(MotionTokens::EasingEmphasizedDecelerateCubicBezier); // 300ms
        k.at(90.0, CircularAnimationAdditionalRotationDelay); // hold till 1500ms
        k.at(180.0, CircularAnimationAdditionalRotationDuration + CircularAnimationAdditionalRotationDelay); // 1800ms
        k.at(180.0, CircularAnimationAdditionalRotationDelay * 2); // hold till 3000ms
        k.at(270.0, CircularAnimationAdditionalRotationDuration + CircularAnimationAdditionalRotationDelay * 2); // 3300ms
        k.at(270.0, CircularAnimationAdditionalRotationDelay * 3); // hold till 4500ms
        k.at(360.0, CircularAnimationAdditionalRotationDuration + CircularAnimationAdditionalRotationDelay * 3); // 4800ms
        k.at(360.0, CircularAnimationProgressDuration); // hold till 6000ms
    });
    InfiniteRepeatableSpec::new(animation, RepeatMode::Restart, StartOffset::default())
}

/// Compose's internal `circularIndeterminateProgressAnimationSpec`: to 87% by 3 s, back to 10% by 6 s.
pub(crate) fn circular_indeterminate_progress_animation_spec() -> InfiniteRepeatableSpec<f32> {
    let animation: KeyframesSpec<f32> = keyframes(|k| {
        k.duration_millis = CircularAnimationProgressDuration; // 6000ms
        k.at(CircularIndeterminateMaxProgress, CircularAnimationProgressDuration / 2).using(CircularProgressEasing); // 3000ms
        k.at(CircularIndeterminateMinProgress, CircularAnimationProgressDuration);
    });
    InfiniteRepeatableSpec::new(animation, RepeatMode::Restart, StartOffset::default())
}

/// A linear line's end going from 0 to 1 after `delay` ms in `duration` ms, of a 1.75 s round (Compose's four
/// `linearIndeterminate…AnimationSpec`s).
fn linear_indeterminate_line_animation_spec(delay: i32, duration: i32) -> InfiniteRepeatableSpec<f32> {
    let animation: KeyframesSpec<f32> = keyframes(|k| {
        k.duration_millis = LinearAnimationDuration;
        k.at(0.0, delay).using(LinearIndeterminateProgressEasing);
        k.at(1.0, duration + delay);
    });
    InfiniteRepeatableSpec::new(animation, RepeatMode::Restart, StartOffset::default())
}

/// Compose's internal `linearIndeterminateFirstLineHeadAnimationSpec`.
pub(crate) fn linear_indeterminate_first_line_head_animation_spec() -> InfiniteRepeatableSpec<f32> {
    linear_indeterminate_line_animation_spec(FirstLineHeadDelay, FirstLineHeadDuration)
}

/// Compose's internal `linearIndeterminateFirstLineTailAnimationSpec`.
pub(crate) fn linear_indeterminate_first_line_tail_animation_spec() -> InfiniteRepeatableSpec<f32> {
    linear_indeterminate_line_animation_spec(FirstLineTailDelay, FirstLineTailDuration)
}

/// Compose's internal `linearIndeterminateSecondLineHeadAnimationSpec`.
pub(crate) fn linear_indeterminate_second_line_head_animation_spec() -> InfiniteRepeatableSpec<f32> {
    linear_indeterminate_line_animation_spec(SecondLineHeadDelay, SecondLineHeadDuration)
}

/// Compose's internal `linearIndeterminateSecondLineTailAnimationSpec`.
pub(crate) fn linear_indeterminate_second_line_tail_animation_spec() -> InfiniteRepeatableSpec<f32> {
    linear_indeterminate_line_animation_spec(SecondLineTailDelay, SecondLineTailDuration)
}

// LinearProgressIndicator Material specs

// Width is given in the spec but not defined as a token.
pub(crate) const LinearIndicatorWidth: Dp = Dp(240.0);

pub(crate) const LinearIndicatorHeight: Dp = LinearProgressIndicatorTokens::Height;

pub(crate) const StopIndicatorTrailingSpace: Dp = Dp(6.0);

// CircularProgressIndicator Material specs
// Diameter of the indicator circle
pub(crate) const CircularIndicatorDiameter: Dp = CircularProgressIndicatorTokens::Size;

// Indeterminate linear indicator transition specs

// Total duration for one linear cycle
pub(crate) const LinearAnimationDuration: i32 = 1750;

// Duration of the head and tail animations for both lines
pub(crate) const FirstLineHeadDuration: i32 = 1000;
pub(crate) const FirstLineTailDuration: i32 = 1000;
pub(crate) const SecondLineHeadDuration: i32 = 850;
pub(crate) const SecondLineTailDuration: i32 = 850;

// Delay before the start of the head and tail animations for both lines
pub(crate) const FirstLineHeadDelay: i32 = 0;
pub(crate) const FirstLineTailDelay: i32 = 250;
pub(crate) const SecondLineHeadDelay: i32 = 650;
pub(crate) const SecondLineTailDelay: i32 = 900;

pub(crate) const LinearIndeterminateProgressEasing: kompoz_animation::CubicBezierEasing =
    MotionTokens::EasingEmphasizedAccelerateCubicBezier;

// Indeterminate circular indicator transition specs

// The indeterminate circular indicator easing constants for its motion
pub(crate) const CircularProgressEasing: kompoz_animation::CubicBezierEasing = MotionTokens::EasingStandardCubicBezier;
pub(crate) const CircularIndeterminateMinProgress: f32 = 0.1;
pub(crate) const CircularIndeterminateMaxProgress: f32 = 0.87;

pub(crate) const CircularAnimationProgressDuration: i32 = 6000;
pub(crate) const CircularAnimationAdditionalRotationDelay: i32 = 1500;
pub(crate) const CircularAnimationAdditionalRotationDuration: i32 = 300;
pub(crate) const CircularAdditionalRotationDegreesTarget: f32 = 360.0;
pub(crate) const CircularGlobalRotationDegreesTarget: f32 = 1080.0;
