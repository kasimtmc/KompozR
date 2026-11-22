//! Material 3 Expressive's wavy progress indicators, as Compose's WavyProgressIndicator.kt (material3 1.5): a bar
//! ([`LinearWavyProgressIndicator`]) and a ring ([`CircularWavyProgressIndicator`]) whose progress waves — its
//! amplitude a function of the progress (by default a wave between 10% and 95%), animated as it changes, the waves
//! moving along — and their endless forms ([`IndeterminateLinearWavyProgressIndicator`],
//! [`IndeterminateCircularWavyProgressIndicator`]); [`WavyProgressIndicatorDefaults`].
//!
//! The drawing is the internal modifiers' (internal/linear_wavy_progress_modifiers.rs,
//! internal/circular_wavy_progress_modifiers.rs).

use crate::internal::accessibility_util::IncreaseVerticalSemanticsBounds;
use crate::internal::circular_wavy_progress_modifiers::CircularWavyProgressModifiers;
use crate::internal::linear_wavy_progress_modifiers::LinearWavyProgressModifiers;
use crate::progress_indicator::{
    fast_coerce_in, linear_indeterminate_first_line_head_animation_spec,
    linear_indeterminate_first_line_tail_animation_spec, linear_indeterminate_second_line_head_animation_spec,
    linear_indeterminate_second_line_tail_animation_spec,
};
use crate::tokens::{
    CircularProgressIndicatorTokens, LinearProgressIndicatorTokens, MotionTokens, ProgressIndicatorTokens,
};
use kompoz_animation::{AnimationSpec, TweenSpec, remember_infinite_transition, tween};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Stroke, StrokeCap, StrokeJoin};
use kompoz_runtime::{kompozable, named_args};
use kompoz_ui::Modifier;
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::semantics::ProgressBarRangeInfo;
use std::rc::Rc;
use std::sync::Arc;

/// A wavy indicator's amplitude of its progress (Compose's `amplitude: (progress: Float) -> Float`): 0 (flat) to 1
/// (the most the indicator's height allows).
pub type AmplitudeFunction = Rc<dyn Fn(f32) -> f32>;

// ---- LinearWavyProgressIndicator --------------------------------------------------------------------------------

/// A determinate linear wavy progress indicator (Compose's `LinearWavyProgressIndicator(progress: () -> Float,
/// …)`): `progress()` of the way in `color` (`stroke`), a wave of `amplitude(progress)` (0..1, animated as it
/// changes) `wavelength` long moving `wave_speed` a second (None: Kotlin's default, `wavelength`: a wavelength a
/// second); its `track_color` track (`track_stroke`) `gap_size` away, ending in a stop indicator of at most
/// `stop_size`. 240 by 10 dp unless the modifier sizes it, clipped to its bounds; its semantics bounds 4 dp taller
/// at each side, a progress in 0..1.
///
/// Kotlin: `LinearWavyProgressIndicator(progress: () -> Float, stopSize: Dp, amplitude: (progress: Float) -> Float)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn LinearWavyProgressIndicator(
    progress: impl Fn() -> f32 + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(WavyProgressIndicatorDefaults::indicator_color())] color: Color,
    #[default(WavyProgressIndicatorDefaults::track_color())] track_color: Color,
    #[default(WavyProgressIndicatorDefaults::linear_indicator_stroke())] stroke: Stroke,
    #[default(WavyProgressIndicatorDefaults::linear_track_stroke())] track_stroke: Stroke,
    #[default(WavyProgressIndicatorDefaults::LinearIndicatorTrackGapSize)] gap_size: Dp,
    #[default(WavyProgressIndicatorDefaults::LinearTrackStopIndicatorSize)] stop_size: Dp,
    #[default(WavyProgressIndicatorDefaults::indicator_amplitude())] amplitude: AmplitudeFunction,
    #[default(WavyProgressIndicatorDefaults::LinearDeterminateWavelength)] wavelength: Dp,
    #[default(None)] wave_speed: Option<Dp>,
) {
    let wave_speed = wave_speed.unwrap_or(wavelength); // Match to 1 wavelength per second
    let progress: Rc<dyn Fn() -> f32> = Rc::new(progress);
    let semantics_progress = progress.clone();
    Spacer(
        modifier
            .then(IncreaseVerticalSemanticsBounds())
            .semantics_with(true, move |c| {
                // Ensure progress lambda is accessed safely for semantics
                let current_progress = Some(semantics_progress()).filter(|p| !p.is_nan()).unwrap_or(0.0);
                c.set_progress_bar_range_info(ProgressBarRangeInfo::new(
                    fast_coerce_in(current_progress, 0.0, 1.0),
                    0.0..=1.0,
                    0,
                ));
            })
            .size_wh(
                WavyProgressIndicatorDefaults::LinearContainerWidth,
                WavyProgressIndicatorDefaults::LinearContainerHeight,
            )
            .clip_to_bounds()
            .linear_wavy_progress_indicator(
                progress,
                amplitude,
                color,
                track_color,
                stroke,
                track_stroke,
                gap_size,
                stop_size,
                wavelength,
                wave_speed,
            ),
    );
}

/// An indeterminate linear wavy progress indicator (Compose's `LinearWavyProgressIndicator(modifier, …,
/// amplitude: Float, …)`): two lines in `color` chasing along a `track_color` track forever (1.75 s a round, as
/// [`IndeterminateLinearProgressIndicator`](crate::IndeterminateLinearProgressIndicator)'s), waving by `amplitude`
/// (kept in 0..1) with waves `wavelength` long moving `wave_speed` a second (None: `wavelength`). It tells
/// accessibility an indeterminate progress.
///
/// Kotlin: `LinearWavyProgressIndicator(modifier: Modifier, amplitude: Float)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn IndeterminateLinearWavyProgressIndicator(
    #[default(Modifier)] modifier: Modifier,
    #[default(WavyProgressIndicatorDefaults::indicator_color())] color: Color,
    #[default(WavyProgressIndicatorDefaults::track_color())] track_color: Color,
    #[default(WavyProgressIndicatorDefaults::linear_indicator_stroke())] stroke: Stroke,
    #[default(WavyProgressIndicatorDefaults::linear_track_stroke())] track_stroke: Stroke,
    #[default(WavyProgressIndicatorDefaults::LinearIndicatorTrackGapSize)] gap_size: Dp,
    #[default(1.0)] amplitude: f32,
    #[default(WavyProgressIndicatorDefaults::LinearIndeterminateWavelength)] wavelength: Dp,
    #[default(None)] wave_speed: Option<Dp>,
) {
    let wave_speed = wave_speed.unwrap_or(wavelength); // Match to 1 wavelength per second
    let infinite_transition = remember_infinite_transition("LinearWavyProgressIndicatorProgress".to_string());
    let first_line_head = named_args!(infinite_transition.animate_float(
        0.0,
        1.0,
        linear_indeterminate_first_line_head_animation_spec(),
        label = "LinearWavyProgressIndicatorFirstHead".to_string()
    ));
    let first_line_tail = named_args!(infinite_transition.animate_float(
        0.0,
        1.0,
        linear_indeterminate_first_line_tail_animation_spec(),
        label = "LinearWavyProgressIndicatorFirstTail".to_string()
    ));
    let second_line_head = named_args!(infinite_transition.animate_float(
        0.0,
        1.0,
        linear_indeterminate_second_line_head_animation_spec(),
        label = "LinearWavyProgressIndicatorSecondHead".to_string()
    ));
    let second_line_tail = named_args!(infinite_transition.animate_float(
        0.0,
        1.0,
        linear_indeterminate_second_line_tail_animation_spec(),
        label = "LinearWavyProgressIndicatorSecondTail".to_string()
    ));

    Spacer(
        modifier
            .then(IncreaseVerticalSemanticsBounds())
            .progress_semantics_indeterminate()
            .size_wh(
                WavyProgressIndicatorDefaults::LinearContainerWidth,
                WavyProgressIndicatorDefaults::LinearContainerHeight,
            )
            .clip_to_bounds()
            .indeterminate_linear_wavy_progress_indicator(
                Rc::new(move || first_line_head.value()),
                Rc::new(move || first_line_tail.value()),
                Rc::new(move || second_line_head.value()),
                Rc::new(move || second_line_tail.value()),
                color,
                track_color,
                stroke,
                track_stroke,
                gap_size,
                fast_coerce_in(amplitude, 0.0, 1.0),
                wavelength,
                wave_speed,
            ),
    );
}

// ---- CircularWavyProgressIndicator ------------------------------------------------------------------------------

/// A determinate circular wavy progress indicator (Compose's `CircularWavyProgressIndicator(progress: () ->
/// Float, …)`): from 12 o'clock clockwise `progress()` of the ring in `color` (`stroke`), waving by
/// `amplitude(progress)` (0..1, animated as it changes) with about `wavelength` long waves (a whole number of them
/// round the ring) moving `wave_speed` a second (None: `wavelength`); the rest of a plain ring in `track_color`
/// (`track_stroke`) `gap_size` away. 48 dp across unless the modifier sizes it; a progress in 0..1 to
/// accessibility.
///
/// Kotlin: `CircularWavyProgressIndicator(progress: () -> Float, amplitude: (progress: Float) -> Float)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn CircularWavyProgressIndicator(
    progress: impl Fn() -> f32 + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(WavyProgressIndicatorDefaults::indicator_color())] color: Color,
    #[default(WavyProgressIndicatorDefaults::track_color())] track_color: Color,
    #[default(WavyProgressIndicatorDefaults::circular_indicator_stroke())] stroke: Stroke,
    #[default(WavyProgressIndicatorDefaults::circular_track_stroke())] track_stroke: Stroke,
    #[default(WavyProgressIndicatorDefaults::CircularIndicatorTrackGapSize)] gap_size: Dp,
    #[default(WavyProgressIndicatorDefaults::indicator_amplitude())] amplitude: AmplitudeFunction,
    #[default(WavyProgressIndicatorDefaults::CircularWavelength)] wavelength: Dp,
    #[default(None)] wave_speed: Option<Dp>,
) {
    let wave_speed = wave_speed.unwrap_or(wavelength); // Match to 1 wavelength per second
    let progress: Rc<dyn Fn() -> f32> = Rc::new(progress);
    let semantics_progress = progress.clone();
    Spacer(
        modifier
            .size(WavyProgressIndicatorDefaults::CircularContainerSize)
            .circular_wavy_progress_indicator(
                progress,
                color,
                track_color,
                stroke,
                track_stroke,
                gap_size,
                amplitude,
                wavelength,
                wave_speed,
            )
            .semantics_with(true, move |c| {
                let progress_value = semantics_progress();
                // Check for NaN, as the ProgressBarRangeInfo will throw an exception.
                let clamped_progress = progress_value.clamp(0.0, 1.0);
                c.set_progress_bar_range_info(ProgressBarRangeInfo::new(
                    if clamped_progress.is_nan() { 0.0 } else { clamped_progress },
                    0.0..=1.0,
                    0,
                ));
            }),
    );
}

/// An indeterminate circular wavy progress indicator (Compose's `CircularWavyProgressIndicator(modifier, …,
/// amplitude: Float, …)`): an arc in `color` growing from a tenth of the ring to 87% and back each 6 s as it turns
/// (as [`IndeterminateCircularProgressIndicator`](crate::IndeterminateCircularProgressIndicator)'s), waving by
/// `amplitude` (kept in 0..1); its `track_color` track the rest of the ring. It tells accessibility an
/// indeterminate progress (Compose's separate spacer for it, b/347736702).
///
/// Kotlin: `CircularWavyProgressIndicator(modifier: Modifier, amplitude: Float)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn IndeterminateCircularWavyProgressIndicator(
    #[default(Modifier)] modifier: Modifier,
    #[default(WavyProgressIndicatorDefaults::indicator_color())] color: Color,
    #[default(WavyProgressIndicatorDefaults::track_color())] track_color: Color,
    #[default(WavyProgressIndicatorDefaults::circular_indicator_stroke())] stroke: Stroke,
    #[default(WavyProgressIndicatorDefaults::circular_track_stroke())] track_stroke: Stroke,
    #[default(WavyProgressIndicatorDefaults::CircularIndicatorTrackGapSize)] gap_size: Dp,
    #[default(1.0)] amplitude: f32,
    #[default(WavyProgressIndicatorDefaults::CircularWavelength)] wavelength: Dp,
    #[default(None)] wave_speed: Option<Dp>,
) {
    let wave_speed = wave_speed.unwrap_or(wavelength); // Match to 1 wavelength per second
    Box(
        modifier = modifier.size(WavyProgressIndicatorDefaults::CircularContainerSize),
        content = move || {
            Spacer(Modifier.fill_max_size().indeterminate_circular_wavy_progress_indicator(
                color,
                track_color,
                stroke.clone(),
                track_stroke.clone(),
                gap_size,
                amplitude,
                wavelength,
                wave_speed,
            ));
            // To overcome b/347736702 we are separating the progressSemantics() call to an independent
            // spacer, and wrap the spacer with the indicator content and this spacer in a Box.
            Spacer(Modifier.fill_max_size().progress_semantics_indeterminate());
        },
    );
}

// ---- WavyProgressIndicatorDefaults ------------------------------------------------------------------------------

/// The wavy progress indicators' defaults (Compose's `WavyProgressIndicatorDefaults`): its properties associated
/// items.
pub struct WavyProgressIndicatorDefaults;

#[allow(non_snake_case)]
impl WavyProgressIndicatorDefaults {
    /// The spec to animate a progress with (Compose's `ProgressAnimationSpec`): 500 ms, linear.
    pub fn ProgressAnimationSpec() -> Arc<dyn AnimationSpec<f32>> {
        let spec: TweenSpec<f32> = named_args!(tween(
            duration_millis = MotionTokens::DurationLong2 as i32,
            easing = Arc::new(MotionTokens::EasingLinearCubicBezier)
        ));
        Arc::new(spec)
    }

    /// The progress's color: the scheme's primary (Compose's `indicatorColor`).
    pub fn indicator_color() -> Color {
        ProgressIndicatorTokens::ActiveIndicatorColor.value()
    }

    /// The track's color: the scheme's secondary container (Compose's `trackColor`).
    pub fn track_color() -> Color {
        ProgressIndicatorTokens::TrackColor.value()
    }

    /// A linear indicator's progress stroke: 4 dp, round (Compose's `linearIndicatorStroke`).
    pub fn linear_indicator_stroke() -> Stroke {
        round_stroke(LinearProgressIndicatorTokens::ActiveThickness)
    }

    /// A circular indicator's progress stroke: 4 dp, round (Compose's `circularIndicatorStroke`).
    pub fn circular_indicator_stroke() -> Stroke {
        round_stroke(CircularProgressIndicatorTokens::ActiveThickness)
    }

    /// A linear indicator's track stroke: 4 dp, round (Compose's `linearTrackStroke`).
    pub fn linear_track_stroke() -> Stroke {
        round_stroke(LinearProgressIndicatorTokens::TrackThickness)
    }

    /// A circular indicator's track stroke: 4 dp, round (Compose's `circularTrackStroke`).
    pub fn circular_track_stroke() -> Stroke {
        round_stroke(CircularProgressIndicatorTokens::TrackThickness)
    }

    /// A determinate linear indicator's wavelength: 40 dp.
    pub const LinearDeterminateWavelength: Dp = LinearProgressIndicatorTokens::ActiveWaveWavelength;

    /// An indeterminate linear indicator's wavelength: 20 dp.
    pub const LinearIndeterminateWavelength: Dp = LinearProgressIndicatorTokens::IndeterminateActiveWaveWavelength;

    /// A linear indicator's height: 10 dp.
    pub const LinearContainerHeight: Dp = LinearProgressIndicatorTokens::WaveHeight;

    /// A linear indicator's width: 240 dp.
    pub const LinearContainerWidth: Dp = Dp(240.0);

    /// A linear indicator's stop indicator size: 4 dp.
    pub const LinearTrackStopIndicatorSize: Dp = LinearProgressIndicatorTokens::StopSize;

    /// A circular indicator's size: 48 dp.
    pub const CircularContainerSize: Dp = CircularProgressIndicatorTokens::WaveSize;

    /// A circular indicator's wavelength: 15 dp.
    pub const CircularWavelength: Dp = CircularProgressIndicatorTokens::ActiveWaveWavelength;

    /// The gap between a linear indicator's progress and its track: 4 dp.
    pub const LinearIndicatorTrackGapSize: Dp = LinearProgressIndicatorTokens::TrackActiveSpace;

    /// The gap between a circular indicator's progress and its track: 4 dp.
    pub const CircularIndicatorTrackGapSize: Dp = CircularProgressIndicatorTokens::TrackActiveSpace;

    /// The amplitude of a progress (Compose's `indicatorAmplitude`): the most from past 10% of the progress to
    /// short of 95%, else none. One function (Kotlin's one lambda instance: an element given it is the same).
    pub fn indicator_amplitude() -> AmplitudeFunction {
        thread_local! {
            static INDICATOR_AMPLITUDE: AmplitudeFunction = Rc::new(|progress: f32| {
                // Sets the amplitude to the max on 10%, and back to zero on 95% of the progress.
                if progress <= 0.1 || progress >= 0.95 { 0.0 } else { 1.0 }
            });
        }
        INDICATOR_AMPLITUDE.with(Rc::clone)
    }
}

/// Compose's `Stroke(width = with(LocalDensity.current) { thickness.toPx() }, cap = StrokeCap.Round)`.
fn round_stroke(thickness: Dp) -> Stroke {
    Stroke::new(
        LocalDensity.current().to_px(thickness),
        Stroke::DefaultMiter,
        StrokeCap::Round,
        StrokeJoin::Miter,
        None,
    )
}

/// Compose's internal `IncreasingAmplitudeAnimationSpec`: an amplitude rising, 500 ms on the standard curve.
pub(crate) fn IncreasingAmplitudeAnimationSpec() -> TweenSpec<f32> {
    named_args!(tween(
        duration_millis = MotionTokens::DurationLong2 as i32,
        easing = Arc::new(MotionTokens::EasingStandardCubicBezier)
    ))
}

/// Compose's internal `DecreasingAmplitudeAnimationSpec`: an amplitude falling, 500 ms on the emphasized
/// accelerating curve.
pub(crate) fn DecreasingAmplitudeAnimationSpec() -> TweenSpec<f32> {
    named_args!(tween(
        duration_millis = MotionTokens::DurationLong2 as i32,
        easing = Arc::new(MotionTokens::EasingEmphasizedAccelerateCubicBezier)
    ))
}
