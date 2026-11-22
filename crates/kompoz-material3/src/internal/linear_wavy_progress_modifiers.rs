//! The drawing of the linear wavy progress indicators, as Compose's internal LinearWavyProgressModifiers.kt: the
//! modifiers ([`LinearWavyProgressModifiers`]) whose nodes draw the track, the progress lines (a wave of the
//! amplitude asked, moving along at its speed) and the stop indicator; the amplitude animated as it changes; the
//! paths cached (a full wave measured once, the parts to draw cut from it with a [`PathMeasure`]).

use crate::progress_indicator::fast_coerce_in;
use crate::wavy_progress_indicator::{DecreasingAmplitudeAnimationSpec, IncreasingAmplitudeAnimationSpec};
use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, InfiniteRepeatableSpec, LinearEasing, RepeatMode, StartOffset,
    TweenSpec, tween,
};
use kompoz_coroutines::Job;
use kompoz_graphics::{
    Color, Dp, DrawStyle, LayoutDirection, Matrix, Offset, Path, PathMeasure, Size, Stroke, StrokeCap,
};
use kompoz_runtime::{MutableState, mutable_float_state_of, named_args};
use kompoz_ui::draw::{CacheDrawModifierNode, CacheDrawScope, ContentDrawScope, DrawScope};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// The linear wavy progress modifiers (Compose's internal `Modifier.linearWavyProgressIndicator` overloads).
#[allow(clippy::too_many_arguments)]
pub(crate) trait LinearWavyProgressModifiers {
    /// Draws a determinate linear wavy progress indicator (Compose's `linearWavyProgressIndicator(progress,
    /// amplitude, …)`): `progress()` of the way in `color` (`stroke`), a wave of `amplitude(progress)` (0..1 of
    /// the most the height allows) `wavelength` long moving `wave_speed` a second, its `track_color` track
    /// (`track_stroke`) `gap_size` away, a stop indicator of at most `stop_size` at its end.
    fn linear_wavy_progress_indicator(
        self,
        progress: Rc<dyn Fn() -> f32>,
        amplitude: Rc<dyn Fn(f32) -> f32>,
        color: Color,
        track_color: Color,
        stroke: Stroke,
        track_stroke: Stroke,
        gap_size: Dp,
        stop_size: Dp,
        wavelength: Dp,
        wave_speed: Dp,
    ) -> Modifier;

    /// Draws an indeterminate linear wavy progress indicator (Compose's `linearWavyProgressIndicator(
    /// firstLineHeadProgress, …)`): two lines between the given heads and tails in `color`, the track in the gaps,
    /// waves of `amplitude` (0..1).
    fn indeterminate_linear_wavy_progress_indicator(
        self,
        first_line_head_progress: Rc<dyn Fn() -> f32>,
        first_line_tail_progress: Rc<dyn Fn() -> f32>,
        second_line_head_progress: Rc<dyn Fn() -> f32>,
        second_line_tail_progress: Rc<dyn Fn() -> f32>,
        color: Color,
        track_color: Color,
        stroke: Stroke,
        track_stroke: Stroke,
        gap_size: Dp,
        amplitude: f32,
        wavelength: Dp,
        wave_speed: Dp,
    ) -> Modifier;
}

impl LinearWavyProgressModifiers for Modifier {
    fn linear_wavy_progress_indicator(
        self,
        progress: Rc<dyn Fn() -> f32>,
        amplitude: Rc<dyn Fn(f32) -> f32>,
        color: Color,
        track_color: Color,
        stroke: Stroke,
        track_stroke: Stroke,
        gap_size: Dp,
        stop_size: Dp,
        wavelength: Dp,
        wave_speed: Dp,
    ) -> Modifier {
        self.then(Modifier::element(DeterminateLinearWavyProgressElement {
            progress,
            amplitude,
            stop_size,
            base: BaseLinearWavyProgressElement {
                color,
                track_color,
                stroke,
                track_stroke,
                gap_size,
                wavelength,
                wave_speed,
            },
        }))
    }

    fn indeterminate_linear_wavy_progress_indicator(
        self,
        first_line_head_progress: Rc<dyn Fn() -> f32>,
        first_line_tail_progress: Rc<dyn Fn() -> f32>,
        second_line_head_progress: Rc<dyn Fn() -> f32>,
        second_line_tail_progress: Rc<dyn Fn() -> f32>,
        color: Color,
        track_color: Color,
        stroke: Stroke,
        track_stroke: Stroke,
        gap_size: Dp,
        amplitude: f32,
        wavelength: Dp,
        wave_speed: Dp,
    ) -> Modifier {
        self.then(Modifier::element(IndeterminateLinearWavyProgressElement {
            first_line_head_progress,
            first_line_tail_progress,
            second_line_head_progress,
            second_line_tail_progress,
            amplitude,
            base: BaseLinearWavyProgressElement {
                color,
                track_color,
                stroke,
                track_stroke,
                gap_size,
                wavelength,
                wave_speed,
            },
        }))
    }
}

// ---- The base element and node -----------------------------------------------------------------------------------

/// What both elements hold (Compose's abstract `BaseLinearWavyProgressElement`): equal by these.
#[derive(Clone, PartialEq)]
struct BaseLinearWavyProgressElement {
    color: Color,
    track_color: Color,
    stroke: Stroke,
    track_stroke: Stroke,
    gap_size: Dp,
    wavelength: Dp,
    wave_speed: Dp,
}

impl BaseLinearWavyProgressElement {
    fn update(&self, node: &Rc<BaseLinearWavyProgressNode>) {
        node.set_color(self.color);
        node.set_track_color(self.track_color);
        node.set_stroke(self.stroke.clone());
        node.set_track_stroke(self.track_stroke.clone());
        node.set_gap_size(self.gap_size);
        node.set_wavelength(self.wavelength);
        node.set_wave_speed(self.wave_speed);
    }

    fn create_base(&self) -> Rc<BaseLinearWavyProgressNode> {
        Rc::new(BaseLinearWavyProgressNode {
            color: Cell::new(self.color),
            track_color: Cell::new(self.track_color),
            stroke: RefCell::new(self.stroke.clone()),
            track_stroke: RefCell::new(self.track_stroke.clone()),
            gap_size: Cell::new(self.gap_size),
            wavelength: Cell::new(self.wavelength),
            wave_speed: Cell::new(self.wave_speed),
            wave_offset: mutable_float_state_of(0.0),
            offset_animation_job: RefCell::new(None),
            amplitude_animatable: RefCell::new(None),
            amplitude_animation_job: RefCell::new(None),
            progress_drawing_cache: RefCell::new(LinearProgressDrawingCache::new()),
            handle: RefCell::new(None),
            cache_draw_node: RefCell::new(None),
        })
    }
}

/// What both nodes share with their draw caches (Compose's abstract `BaseLinearWavyProgressNode`, a
/// DelegatingNode): its parameters (a change of color drawn again, of a size the cache built again, of the wave
/// the offset animation started again), the wave offset animation, the amplitude animation, the drawing cache. Its
/// draw cache node is delegated to as it is first attached (a node has its context then); `invalidateDraw` and
/// `invalidateDrawCache` are that node's.
struct BaseLinearWavyProgressNode {
    color: Cell<Color>,
    track_color: Cell<Color>,
    stroke: RefCell<Stroke>,
    track_stroke: RefCell<Stroke>,
    gap_size: Cell<Dp>,
    wavelength: Cell<Dp>,
    wave_speed: Cell<Dp>,
    wave_offset: MutableState<f32>,
    offset_animation_job: RefCell<Option<Job>>,
    amplitude_animatable: RefCell<Option<Animatable<f32, AnimationVector1D>>>,
    amplitude_animation_job: RefCell<Option<Job>>,
    progress_drawing_cache: RefCell<LinearProgressDrawingCache>,
    handle: RefCell<Option<NodeHandle>>,
    cache_draw_node: RefCell<Option<DelegatedNode<CacheDrawModifierNode>>>,
}

impl BaseLinearWavyProgressNode {
    fn set_color(&self, value: Color) {
        if self.color.get() != value {
            self.color.set(value);
            self.invalidate_draw();
        }
    }

    fn set_track_color(&self, value: Color) {
        if self.track_color.get() != value {
            self.track_color.set(value);
            self.invalidate_draw();
        }
    }

    fn set_stroke(&self, value: Stroke) {
        if *self.stroke.borrow() != value {
            *self.stroke.borrow_mut() = value;
            self.invalidate_draw_cache();
        }
    }

    fn set_track_stroke(&self, value: Stroke) {
        if *self.track_stroke.borrow() != value {
            *self.track_stroke.borrow_mut() = value;
            self.invalidate_draw_cache();
        }
    }

    fn set_gap_size(&self, value: Dp) {
        if self.gap_size.get() != value {
            self.gap_size.set(value);
            self.invalidate_draw_cache();
        }
    }

    fn set_wavelength(self: &Rc<Self>, value: Dp) {
        if self.wavelength.get() != value {
            self.wavelength.set(value);
            self.update_offset_animation(); // Restart animation
            self.invalidate_draw_cache();
        }
    }

    fn set_wave_speed(self: &Rc<Self>, value: Dp) {
        if self.wave_speed.get() != value {
            self.wave_speed.set(value);
            self.update_offset_animation(); // Restart animation
            self.invalidate_draw_cache();
        }
    }

    fn is_attached(&self) -> bool {
        self.handle.borrow().as_ref().is_some_and(NodeHandle::is_attached)
    }

    /// Compose's `onAttach`: potentially start the wave offset animation.
    fn on_attach(self: &Rc<Self>) {
        self.update_offset_animation();
    }

    /// Compose's `onDetach`.
    fn on_detach(&self) {
        *self.amplitude_animatable.borrow_mut() = None;
    }

    /// Starts the wave offset animation (Compose's `updateOffsetAnimation`): a wave length every `wavelength /
    /// wave_speed` seconds (at least 50 ms) from where it is, forever; none (the offset 0) without a speed or a
    /// wavelength.
    fn update_offset_animation(self: &Rc<Self>) {
        if let Some(job) = self.offset_animation_job.borrow_mut().take() {
            job.cancel();
        }

        if !self.is_attached() {
            return;
        }

        let (wave_speed, wavelength) = (self.wave_speed.get(), self.wavelength.get());
        if wave_speed > Dp(0.0) && wavelength > Dp(0.0) {
            let duration_millis = fast_round_to_int((wavelength.0 / wave_speed.0) * 1000.0).max(MinAnimationDuration);
            let Some(scope) = self.handle.borrow().as_ref().and_then(NodeHandle::coroutine_scope) else { return };
            let wave_offset = self.wave_offset.clone();
            let job = named_args!(scope.launch(async move {
                // Start from current offset
                let start_value = wave_offset.peek();
                let offset_animatable = Animatable(start_value);
                let end_value = start_value + 1.0;
                named_args!(offset_animatable.update_bounds(Some(start_value), Some(end_value)));
                let animation: TweenSpec<f32> =
                    named_args!(tween(duration_millis = duration_millis, easing = Arc::new(LinearEasing)));
                let spec: Arc<dyn AnimationSpec<f32>> =
                    Arc::new(InfiniteRepeatableSpec::new(animation, RepeatMode::Restart, StartOffset::default()));
                let offset = wave_offset.clone();
                named_args!(offset_animatable.animate_to(
                    end_value,
                    animation_spec = spec,
                    block = on_frame(move |a: &Animatable<f32, AnimationVector1D>| { offset.set(a.value() % 1.0) })
                ))
                .await;
            }));
            *self.offset_animation_job.borrow_mut() = Some(job);
        } else {
            // No speed or wavelength, so we ensure that the offset is snapped to 0
            self.wave_offset.set(0.0);
        }
    }

    /// Animates the amplitude to `target_amplitude_px` (Compose's `updateAmplitudeAnimation`): made at it the
    /// first time; a new target taken only once an animation running to another is done, rising on the standard
    /// curve, falling on the emphasized accelerating one.
    fn update_amplitude_animation(&self, target_amplitude_px: f32) {
        let current_amplitude_animatable =
            self.amplitude_animatable.borrow_mut().get_or_insert_with(|| Animatable(target_amplitude_px)).clone();

        // Check if the amplitude target has changed AND animation is not already running
        // towards it. If so, launch the amplitude animation within the node's scope
        let job_done = self.amplitude_animation_job.borrow().as_ref().is_none_or(Job::is_completed);
        if self.is_attached() && current_amplitude_animatable.target_value() != target_amplitude_px && job_done {
            let Some(scope) = self.handle.borrow().as_ref().and_then(NodeHandle::coroutine_scope) else { return };
            let job = named_args!(scope.launch(async move {
                let spec: Arc<dyn AnimationSpec<f32>> = if current_amplitude_animatable.peek() < target_amplitude_px {
                    Arc::new(IncreasingAmplitudeAnimationSpec())
                } else {
                    Arc::new(DecreasingAmplitudeAnimationSpec())
                };
                named_args!(current_amplitude_animatable.animate_to(target_amplitude_px, animation_spec = spec)).await;
            }));
            *self.amplitude_animation_job.borrow_mut() = Some(job);
        }
    }

    /// Draws again (Compose's `invalidateDraw`: the cache node's).
    fn invalidate_draw(&self) {
        if let Some(h) = self.handle.borrow().as_ref() {
            h.invalidate_draw();
        }
    }

    /// Builds the cache again as it is next drawn (Compose's `invalidateDrawCache`: the cache node's).
    fn invalidate_draw_cache(&self) {
        let cache = self.cache_draw_node.borrow().clone();
        if let Some(cache) = cache {
            cache.borrow_mut().invalidate_draw_cache();
        }
    }

    /// Attaches the node: its handle kept, its draw cache delegated to the first time.
    fn attach(self: &Rc<Self>, cx: &NodeCx, cache: impl FnOnce() -> CacheDrawModifierNode) {
        *self.handle.borrow_mut() = Some(cx.handle());
        if self.cache_draw_node.borrow().is_none() {
            let delegated = cx.delegate(cache());
            *self.cache_draw_node.borrow_mut() = Some(delegated);
        }
        self.on_attach();
    }

    /// The track, the progress paths and (`stop`) the stop indicator, drawn as they are now (the body of both
    /// nodes' `onDrawWithContent`): the cache's paths brought up to date first.
    fn draw(&self, c: &mut ContentDrawScope, progress_fractions: &[f32], stop: Option<(f32, Dp)>) {
        // Read from the states here and update the LinearProgressDrawingCache inside
        // the onDrawWithContent to avoid voiding the cache at this
        // CacheDrawModifierNode.
        let current_amplitude = self.amplitude_animatable.borrow().as_ref().map_or(0.0, |a| a.value());
        let (stroke, track_stroke) = (self.stroke.borrow().clone(), self.track_stroke.borrow().clone());
        let size = c.size();
        let wavelength_px = c.to_px(self.wavelength.get());
        let gap_size_px = c.to_px(self.gap_size.get());
        let mut cache = self.progress_drawing_cache.borrow_mut();
        cache.update_paths(
            size,
            wavelength_px,
            progress_fractions,
            current_amplitude,
            if current_amplitude > 0.0 { self.wave_offset.value() } else { 0.0 },
            gap_size_px,
            &stroke,
            &track_stroke,
        );

        // Draw
        let (color, track_color) = (self.color.get(), self.track_color.get());
        let degrees = if c.layout_direction() == LayoutDirection::Ltr { 0.0 } else { 180.0 };
        let pivot = c.center();
        c.rotate_with(degrees, pivot, |c| {
            // Draw track
            named_args!(c.draw_path(
                &cache.track_path_to_draw,
                track_color,
                style = DrawStyle::Stroke(track_stroke.clone())
            ));

            // Draw the progress
            if let Some(progress_paths) = &cache.progress_paths_to_draw {
                for path in progress_paths {
                    named_args!(c.draw_path(path, color, style = DrawStyle::Stroke(stroke.clone())));
                }
            }

            // Draw a stop indicator
            if let Some((progress_end, stop_size)) = stop {
                draw_stop_indicator(
                    c,
                    progress_end,
                    size,
                    stop_size,
                    cache.current_stroke_cap_width,
                    &track_stroke,
                    color,
                );
            }
        });
    }
}

// ---- The determinate element and node ----------------------------------------------------------------------------

/// Compose's private `DeterminateLinearWavyProgressElement`: equal by its parameters, the lambdas by identity.
struct DeterminateLinearWavyProgressElement {
    progress: Rc<dyn Fn() -> f32>,
    amplitude: Rc<dyn Fn(f32) -> f32>,
    stop_size: Dp,
    base: BaseLinearWavyProgressElement,
}

impl PartialEq for DeterminateLinearWavyProgressElement {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
            && self.stop_size == other.stop_size
            && Rc::ptr_eq(&self.progress, &other.progress)
            && Rc::ptr_eq(&self.amplitude, &other.amplitude)
    }
}

impl ModifierNodeElement for DeterminateLinearWavyProgressElement {
    type Node = DeterminateLinearWavyProgressNode;

    fn create(&self) -> DeterminateLinearWavyProgressNode {
        DeterminateLinearWavyProgressNode {
            base: self.base.create_base(),
            own: Rc::new(DeterminateOwn {
                progress: RefCell::new(self.progress.clone()),
                amplitude: RefCell::new(self.amplitude.clone()),
                stop_size: Cell::new(self.stop_size),
            }),
        }
    }

    fn update(&self, node: &mut DeterminateLinearWavyProgressNode) {
        self.base.update(&node.base);
        node.set_stop_size(self.stop_size);
        let changed = !Rc::ptr_eq(&node.own.progress.borrow(), &self.progress)
            || !Rc::ptr_eq(&node.own.amplitude.borrow(), &self.amplitude);
        if changed {
            *node.own.progress.borrow_mut() = self.progress.clone();
            *node.own.amplitude.borrow_mut() = self.amplitude.clone();
            node.base.invalidate_draw_cache();
        }
    }

    fn name(&self) -> &'static str {
        "determinateLinearWavyProgressIndicator"
    }
}

/// The determinate node's own parameters, shared with its draw cache.
struct DeterminateOwn {
    progress: RefCell<Rc<dyn Fn() -> f32>>,
    amplitude: RefCell<Rc<dyn Fn(f32) -> f32>>,
    stop_size: Cell<Dp>,
}

impl DeterminateOwn {
    /// Compose's `getProgressFractions`: from 0 to the progress (kept in 0..1).
    fn get_progress_fractions(&self) -> [f32; 2] {
        let progress = self.progress.borrow().clone();
        [0.0, fast_coerce_in(progress(), 0.0, 1.0)] // Start is always 0f for determinate
    }
}

/// Compose's private `DeterminateLinearWavyProgressNode`.
struct DeterminateLinearWavyProgressNode {
    base: Rc<BaseLinearWavyProgressNode>,
    own: Rc<DeterminateOwn>,
}

impl DeterminateLinearWavyProgressNode {
    fn set_stop_size(&self, value: Dp) {
        if self.own.stop_size.get() != value {
            self.own.stop_size.set(value);
            self.base.invalidate_draw();
        }
    }

    /// Compose's `cacheDrawNodeDelegate`: the amplitude's target (of the progress) animated to, then each frame
    /// the paths and the stop indicator drawn.
    fn cache_draw_node(&self) -> CacheDrawModifierNode {
        let (base, own) = (self.base.clone(), self.own.clone());
        CacheDrawModifierNode(move |_scope: &mut CacheDrawScope| {
            let progress = own.progress.borrow().clone();
            let amplitude = own.amplitude.borrow().clone();
            let coerced_progress = fast_coerce_in(progress(), 0.0, 1.0);
            let target_amplitude_px = fast_coerce_in(amplitude(coerced_progress), 0.0, 1.0);

            // In case the amplitude was updated, update the animation that will transition it
            // smoothly.
            base.update_amplitude_animation(target_amplitude_px);

            let (base, own) = (base.clone(), own.clone());
            _scope.on_draw_with_content(move |c| {
                let fractions = own.get_progress_fractions();
                let progress_end = own.get_progress_fractions()[1];
                base.draw(c, &fractions, Some((progress_end, own.stop_size.get())));
            })
        })
    }
}

impl ModifierNode for DeterminateLinearWavyProgressNode {
    node_capabilities!();

    fn on_attach(&mut self, cx: &NodeCx) {
        let cache = || self.cache_draw_node();
        self.base.attach(cx, cache);
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.base.on_detach();
    }
}

// ---- The indeterminate element and node --------------------------------------------------------------------------

/// Compose's private `IndeterminateLinearWavyProgressElement`: equal by its parameters, the lambdas left out.
struct IndeterminateLinearWavyProgressElement {
    first_line_head_progress: Rc<dyn Fn() -> f32>,
    first_line_tail_progress: Rc<dyn Fn() -> f32>,
    second_line_head_progress: Rc<dyn Fn() -> f32>,
    second_line_tail_progress: Rc<dyn Fn() -> f32>,
    amplitude: f32,
    base: BaseLinearWavyProgressElement,
}

impl PartialEq for IndeterminateLinearWavyProgressElement {
    fn eq(&self, other: &Self) -> bool {
        // Exclude the lambdas for head and tail progress
        self.base == other.base && self.amplitude == other.amplitude
    }
}

impl ModifierNodeElement for IndeterminateLinearWavyProgressElement {
    type Node = IndeterminateLinearWavyProgressNode;

    fn create(&self) -> IndeterminateLinearWavyProgressNode {
        IndeterminateLinearWavyProgressNode {
            base: self.base.create_base(),
            own: Rc::new(IndeterminateOwn {
                first_line_head_progress: RefCell::new(self.first_line_head_progress.clone()),
                first_line_tail_progress: RefCell::new(self.first_line_tail_progress.clone()),
                second_line_head_progress: RefCell::new(self.second_line_head_progress.clone()),
                second_line_tail_progress: RefCell::new(self.second_line_tail_progress.clone()),
                amplitude: Cell::new(fast_coerce_in(self.amplitude, 0.0, 1.0)),
            }),
        }
    }

    fn update(&self, node: &mut IndeterminateLinearWavyProgressNode) {
        self.base.update(&node.base);
        *node.own.first_line_head_progress.borrow_mut() = self.first_line_head_progress.clone();
        *node.own.first_line_tail_progress.borrow_mut() = self.first_line_tail_progress.clone();
        *node.own.second_line_head_progress.borrow_mut() = self.second_line_head_progress.clone();
        *node.own.second_line_tail_progress.borrow_mut() = self.second_line_tail_progress.clone();
        node.set_amplitude(self.amplitude);
    }

    fn name(&self) -> &'static str {
        "indeterminateLinearWavyProgressIndicator"
    }
}

/// The indeterminate node's own parameters, shared with its draw cache.
struct IndeterminateOwn {
    first_line_head_progress: RefCell<Rc<dyn Fn() -> f32>>,
    first_line_tail_progress: RefCell<Rc<dyn Fn() -> f32>>,
    second_line_head_progress: RefCell<Rc<dyn Fn() -> f32>>,
    second_line_tail_progress: RefCell<Rc<dyn Fn() -> f32>>,
    amplitude: Cell<f32>,
}

impl IndeterminateOwn {
    /// Compose's `getProgressFractions`: each line's tail and head.
    fn get_progress_fractions(&self) -> [f32; 4] {
        let read = |f: &RefCell<Rc<dyn Fn() -> f32>>| {
            let f = f.borrow().clone();
            f()
        };
        [
            read(&self.first_line_tail_progress),
            read(&self.first_line_head_progress),
            read(&self.second_line_tail_progress),
            read(&self.second_line_head_progress),
        ]
    }
}

/// Compose's private `IndeterminateLinearWavyProgressNode`.
struct IndeterminateLinearWavyProgressNode {
    base: Rc<BaseLinearWavyProgressNode>,
    own: Rc<IndeterminateOwn>,
}

impl IndeterminateLinearWavyProgressNode {
    fn set_amplitude(&self, value: f32) {
        let coerced = fast_coerce_in(value, 0.0, 1.0);
        if self.own.amplitude.get() != coerced {
            self.own.amplitude.set(coerced);
            self.base.invalidate_draw_cache();
        }
    }

    /// Compose's `cacheDrawNodeDelegate`: the amplitude animated to, then each frame the paths drawn.
    fn cache_draw_node(&self) -> CacheDrawModifierNode {
        let (base, own) = (self.base.clone(), self.own.clone());
        CacheDrawModifierNode(move |scope: &mut CacheDrawScope| {
            // In case the amplitude was updated, update the animation that will transition it
            // smoothly.
            base.update_amplitude_animation(own.amplitude.get());

            let (base, own) = (base.clone(), own.clone());
            scope.on_draw_with_content(move |c| {
                let fractions = own.get_progress_fractions();
                base.draw(c, &fractions, None);
            })
        })
    }
}

impl ModifierNode for IndeterminateLinearWavyProgressNode {
    node_capabilities!();

    fn on_attach(&mut self, cx: &NodeCx) {
        let cache = || self.cache_draw_node();
        self.base.attach(cx, cache);
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.base.on_detach();
    }
}

// ---- The stop indicator and the drawing cache --------------------------------------------------------------------

/// Draws a stop indicator at the end of the track (Compose's private `DrawScope.drawStopIndicator`): no larger
/// than the track's stroke (a quarter of it in from the end if smaller), shrinking as the progress reaches it.
fn draw_stop_indicator(
    d: &mut DrawScope,
    progress_end: f32,
    progress_indicator_size: Size,
    max_stop_indicator_size: Dp,
    horizontal_insets: f32,
    track_stroke: &Stroke,
    color: Color,
) {
    let mut stop_indicator_size = track_stroke.width.min(d.to_px(max_stop_indicator_size));
    // This will add an additional offset to the indicator's position in case the height of the
    // progress bar is greater than the height of the indicator.
    let indicator_x_offset = if stop_indicator_size == track_stroke.width { 0.0 } else { track_stroke.width / 4.0 };

    let mut indicator_x = progress_indicator_size.width - stop_indicator_size - indicator_x_offset;
    let progress_x = progress_indicator_size.width * progress_end + horizontal_insets;
    if indicator_x <= progress_x {
        stop_indicator_size = 0f32.max(stop_indicator_size - (progress_x - indicator_x));
        indicator_x = progress_x;
    }

    if stop_indicator_size > 0.0 {
        if track_stroke.cap == StrokeCap::Round {
            named_args!(d.draw_circle(
                color,
                radius = stop_indicator_size / 2.0,
                center = Offset::new(indicator_x + stop_indicator_size / 2.0, progress_indicator_size.height / 2.0)
            ));
        } else {
            named_args!(d.draw_rect(
                color,
                top_left = Offset::new(indicator_x, (progress_indicator_size.height - stop_indicator_size) / 2.0),
                size = Size::new(stop_indicator_size, stop_indicator_size)
            ));
        }
    }
}

/// The paths a linear wavy indicator draws, made again only as what they depend on changes (Compose's private
/// `LinearProgressDrawingCache`): a full wave (or line) across the width and two wavelengths more, measured; the
/// progress parts cut from it (shifted by the offset, flattened to the amplitude); the track in the gaps.
struct LinearProgressDrawingCache {
    current_wavelength: f32,
    current_amplitude: f32,
    current_size: Size,
    current_progress_fractions: Option<Vec<f32>>,
    current_indicator_track_gap_size: f32,
    current_wave_offset: f32,
    current_stroke: Stroke,
    current_track_stroke: Stroke,

    // This scale value is used to grab segments from the pathMeasure in the correct length.
    // It holds a value that is the result of dividing the PathMeasure length by the actual Path
    // width (in pixels) that it's holding. When the amplitude is zero and the line is flat, the
    // scale would be 1.0. However, when the amplitude is greater than zero, the path is wavy and
    // its measured length in the PathMeasure would be longer than its measured width on screen, so
    // the scale would be greater than 1.0.
    progress_path_scale: f32,

    /// The full path: the whole width and a couple of waves more, at the most amplitude the height allows.
    full_progress_path: Path,
    /// What measures the full path.
    path_measure: PathMeasure,
    /// The track to draw.
    track_path_to_draw: Path,
    /// The progress parts to draw (one a pair of fractions).
    progress_paths_to_draw: Option<Vec<Path>>,
    /// The room the caps need at each end.
    current_stroke_cap_width: f32,
}

impl LinearProgressDrawingCache {
    fn new() -> LinearProgressDrawingCache {
        let current_stroke = Stroke::default();
        LinearProgressDrawingCache {
            current_wavelength: -1.0,
            current_amplitude: -1.0,
            current_size: Size::Unspecified,
            current_progress_fractions: None,
            current_indicator_track_gap_size: 0.0,
            current_wave_offset: -1.0,
            current_track_stroke: current_stroke.clone(),
            current_stroke,
            progress_path_scale: 1.0,
            full_progress_path: Path::new(),
            path_measure: PathMeasure::new(),
            track_path_to_draw: Path::new(),
            progress_paths_to_draw: None,
            current_stroke_cap_width: 0.0,
        }
    }

    /// Brings the paths up to date (Compose's `updatePaths`): the full path when the size, wavelength, strokes, gap
    /// change or the amplitude turns to or from 0, the parts to draw when the fractions, amplitude or offset do.
    #[allow(clippy::too_many_arguments)]
    fn update_paths(
        &mut self,
        size: Size,
        wavelength: f32,
        progress_fractions: &[f32],
        amplitude: f32,
        wave_offset: f32,
        gap_size: f32,
        stroke: &Stroke,
        track_stroke: &Stroke,
    ) {
        if self.current_progress_fractions.is_none() {
            // Just create FloatArray to match the size of the given progressFractions array.
            // We will later call copyInto to copy the incoming values into this cache.
            self.current_progress_fractions = Some(vec![0.0; progress_fractions.len()]);
            self.progress_paths_to_draw = Some((0..progress_fractions.len() / 2).map(|_| Path::new()).collect());
        }
        let paths_updates = self.update_full_paths(size, wavelength, amplitude, gap_size, stroke, track_stroke);
        self.update_draw_paths(paths_updates, progress_fractions, amplitude, wave_offset);
    }

    /// Compose's private `updateFullPaths`: whether the full path was made again.
    fn update_full_paths(
        &mut self,
        size: Size,
        wavelength: f32,
        amplitude: f32,
        gap_size: f32,
        stroke: &Stroke,
        track_stroke: &Stroke,
    ) -> bool {
        if self.current_size == size
            && self.current_wavelength == wavelength
            && self.current_stroke == *stroke
            && self.current_track_stroke == *track_stroke
            && self.current_indicator_track_gap_size == gap_size
            // Check if an amplitude change should trigger a full path update. If the amplitude
            // is turning to or from zero, this will trigger an update.
            && ((self.current_amplitude != 0.0 && amplitude != 0.0)
                || self.current_amplitude == 0.0 && amplitude == 0.0)
        {
            // No update required
            return false;
        }

        let height = size.height;
        let width = size.width;

        // Update the Stroke cap width to take into consideration when drawing the Path.
        self.current_stroke_cap_width =
            if (stroke.cap == StrokeCap::Butt && track_stroke.cap == StrokeCap::Butt) || height > width {
                0.0
            } else {
                (stroke.width / 2.0).max(track_stroke.width / 2.0)
            };

        // There are changes that should update the full path.
        let path = &mut self.full_progress_path;
        path.rewind();
        path.move_to(0.0, 0.0);

        if amplitude == 0.0 {
            // Just a line in this case, so we can optimize with a simple lineTo call.
            path.line_to(width, 0.0);
        } else {
            let half_wavelength_px = wavelength / 2.0;
            let mut anchor_x = half_wavelength_px;
            let anchor_y = 0.0;
            let mut control_x = half_wavelength_px / 2.0;

            // We set the amplitude to the max available height to create a sine-like path that will
            // later be Y-scaled on draw.
            // Note that with quadratic plotting, the height of the control point, when
            // perpendicular to the center point between the anchors, will plot a wave that peaks at
            // half the height.
            // We offset this height with the progress stroke's width to avoid cropping the drawing
            // later.
            let mut control_y = height - stroke.width;

            // Plot a path that holds a couple of extra waves. This can later be used to create a
            // progressPathToDraw with a wave offset value to simulate a wave movement.
            // Note that we add more than one wave-length to support cases where the wavelength is
            // relatively large and may end up in cases where a single extra wavelength is not
            // sufficient for the wave's motion drawing.
            let width_with_extra_phase = width + wavelength * 2.0;
            while anchor_x <= width_with_extra_phase {
                path.quadratic_to(control_x, control_y, anchor_x, anchor_y);
                anchor_x += half_wavelength_px;
                control_x += half_wavelength_px;
                control_y *= -1.0;
            }
        }

        path.translate(Offset::new(0.0, height / 2.0));

        // Update the PathMeasure with the full path
        self.path_measure.set_path(&self.full_progress_path, false);

        // Calculate the progressPathScale by dividing the length of the path that the PathMeasure
        // holds by its actual width in pixels. We will use this scale value later when grabbing
        // segments from the pathMeasure.
        let full_path_length = self.path_measure.length();
        self.progress_path_scale = full_path_length / (self.full_progress_path.get_bounds().width() + 0.000_000_01);

        // Cache the full path attributes (note that the amplitude is intentionally not cached here,
        // and will be cached on the updateDrawPaths call)
        self.current_size = size;
        self.current_wavelength = wavelength;
        self.current_stroke = stroke.clone();
        self.current_track_stroke = track_stroke.clone();
        self.current_indicator_track_gap_size = gap_size;
        true
    }

    /// Compose's private `updateDrawPaths`: the progress parts cut from the full path, shifted and flattened; the
    /// track drawn right to left in the gaps between them.
    fn update_draw_paths(&mut self, force_update: bool, progress_fractions: &[f32], amplitude: f32, wave_offset: f32) {
        assert!(!self.current_size.is_unspecified(), "updateDrawPaths was called before updateFullPaths");
        let expected = self.progress_paths_to_draw.as_ref().map_or(0, Vec::len);
        assert!(
            expected == progress_fractions.len() / 2,
            "the given progress fraction pairs do not match the expected number of progress paths to draw. \
             updateDrawPaths called with {} pairs, while there are {} expected progress paths.",
            progress_fractions.len() / 2,
            expected
        );
        if !force_update
            && self.current_progress_fractions.as_deref() == Some(progress_fractions)
            && self.current_amplitude == amplitude
            && self.current_wave_offset == wave_offset
        {
            // No update required
            return;
        }
        let width = self.current_size.width;
        let half_height = self.current_size.height / 2.0;
        let cap = self.current_stroke_cap_width;

        let mut adjusted_track_gap_size = self.current_indicator_track_gap_size;

        // The path will only be visible if the Cap can be drawn, so this flag will indicate when
        // that happens to help us adjust the gap between the active indicator and the track.
        let mut active_indicator_visible = false;

        // For each of the progress paths, apply a segment from the PathMeasure that was previously
        // created for the entire width of the progress bar. Also, draw the track parts in the gaps
        // between the progress parts.
        let mut next_end_track_offset = width - cap;
        self.track_path_to_draw.rewind();
        self.track_path_to_draw.move_to(next_end_track_offset, half_height);
        let paths = self.progress_paths_to_draw.as_mut().expect("the progress paths");
        for (i, path) in paths.iter_mut().enumerate() {
            path.rewind();

            let start_progress_fraction = progress_fractions[i * 2];
            let end_progress_fraction = progress_fractions[i * 2 + 1];

            let bar_tail = start_progress_fraction * width;
            let bar_head = end_progress_fraction * width;

            if i == 0 {
                // Potentially shorten the gap and insets when the progress bar just enters the
                // track.
                // When rounded caps are applied, we need enough space to draw the initial path
                // (i.e. circle), so by only adjusting the gap size when the
                // barHead >= currentStrokeCapWidth we ensure that the track is not being shortened
                // in this initial progress phase.
                adjusted_track_gap_size = if bar_head < cap {
                    0.0 // barHead
                } else {
                    (bar_head - cap).min(self.current_indicator_track_gap_size /*+ currentStrokeCapWidth * 2*/)
                };
                active_indicator_visible = bar_head >= cap;
            }
            // Coerce the bar's head and tail to ensure we leave room for the drawing of the
            // stroke's caps.
            let adjusted_bar_head = fast_coerce_in(bar_head, cap, width - cap);
            let adjusted_bar_tail = fast_coerce_in(bar_tail, cap, width - cap);

            // Update the progressPathToDraw
            if (end_progress_fraction - start_progress_fraction).abs() > 0.0 {
                // For flat lines (i.e. amplitude == 0), there is no need to offset the wave.
                let wave_shift = if amplitude != 0.0 { wave_offset * self.current_wavelength } else { 0.0 };
                self.path_measure.get_segment(
                    (adjusted_bar_tail + wave_shift) * self.progress_path_scale,
                    (adjusted_bar_head + wave_shift) * self.progress_path_scale,
                    path,
                    true,
                );

                // Translate and scale the draw path by the wave shift and the amplitude.
                let mut m = Matrix::new();
                m.translate(if wave_shift > 0.0 { -wave_shift } else { 0.0 }, (1.0 - amplitude) * half_height, 0.0);
                // The progressPathToDraw is a segment of the full progress path, which is
                // always in the maximum possible amplitude. This scaling will flatten the
                // wave to the given amplitude percentage.
                if amplitude != 1.0 {
                    m.scale(1.0, amplitude, 1.0);
                }
                path.transform(&m);
            }

            // While we draw the progress parts from left to right, we also draw the track parts
            // from right to left and update the nextEndTrackOffset on every pass.
            // Before that, we calculate the spacing between the active indicator and the track to
            // adjust it if needed when the progress is small.
            let adaptive_track_spacing =
                if active_indicator_visible { adjusted_track_gap_size + cap * 2.0 } else { adjusted_track_gap_size };
            if next_end_track_offset > adjusted_bar_head + adaptive_track_spacing {
                self.track_path_to_draw.line_to(cap.max(adjusted_bar_head + adaptive_track_spacing), half_height);
            }

            if bar_head > bar_tail {
                // Update the nextEndTrackOffset and move the path to prepare for the next draw.
                next_end_track_offset = cap.max(adjusted_bar_tail - adaptive_track_spacing);
                self.track_path_to_draw.move_to(next_end_track_offset, half_height);
            }
        }

        // Final track drawing, if needed. This will fill any track gaps to the left of the
        // progress.
        if next_end_track_offset > cap {
            self.track_path_to_draw.line_to(cap, half_height);
        }

        // Cache
        if let Some(current) = self.current_progress_fractions.as_mut() {
            current.copy_from_slice(progress_fractions);
        }
        self.current_amplitude = amplitude;
        self.current_wave_offset = wave_offset;
    }
}

/// What an animation calls each frame with its animatable (an `animateTo` block).
pub(crate) type OnFrame = Box<dyn for<'a> FnMut(&'a Animatable<f32, AnimationVector1D>)>;

/// `block` as an `animateTo` block.
pub(crate) fn on_frame(block: impl FnMut(&Animatable<f32, AnimationVector1D>) + 'static) -> Option<OnFrame> {
    Some(Box::new(block))
}

/// Compose's `fastRoundToInt` (Kotlin's `roundToInt`: a half up).
pub(crate) fn fast_round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

// Minimum animation duration to ensure we don't overwhelm the CPU.
const MinAnimationDuration: i32 = 50;
