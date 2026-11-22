//! The drawing of the circular wavy progress indicators, as Compose's internal CircularWavyProgressModifiers.kt:
//! the modifiers ([`CircularWavyProgressModifiers`]) whose nodes draw a ring whose progress part waves (a star of
//! rounded points as many as the circle holds wavelengths, morphed from a circle by the amplitude) and turns along
//! at its speed, its track the rest of a plain ring; the amplitude animated as it changes; an endless one turning
//! and growing and shrinking as Compose's indeterminate circular indicator; the paths cached and cut with a
//! [`PathMeasure`].

use crate::internal::linear_wavy_progress_modifiers::{fast_round_to_int, on_frame};
use crate::progress_indicator::{
    CircularAdditionalRotationDegreesTarget, CircularGlobalRotationDegreesTarget, CircularIndeterminateMaxProgress,
    CircularIndeterminateMinProgress, circular_indeterminate_global_rotation_animation_spec,
    circular_indeterminate_progress_animation_spec, circular_indeterminate_rotation_animation_spec, fast_coerce_in,
};
use crate::wavy_progress_indicator::{DecreasingAmplitudeAnimationSpec, IncreasingAmplitudeAnimationSpec};
use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, InfiniteRepeatableSpec, LinearEasing, RepeatMode, StartOffset,
    TweenSpec, tween,
};
use kompoz_coroutines::Job;
use kompoz_graphics::material_shapes::{CornerRounding, Morph, RoundedPolygon};
use kompoz_graphics::{Color, Dp, DrawStyle, Matrix, Offset, Path, PathMeasure, Size, Stroke, StrokeCap};
use kompoz_runtime::{MutableState, mutable_float_state_of, mutable_int_state_of, named_args};
use kompoz_ui::draw::{CacheDrawModifierNode, CacheDrawScope, DrawScope};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::{Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities};
use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;
use std::sync::Arc;

/// The circular wavy progress modifiers (Compose's internal `Modifier.circularWavyProgressIndicator` overloads).
#[allow(clippy::too_many_arguments)]
pub(crate) trait CircularWavyProgressModifiers {
    /// Draws a determinate circular wavy progress indicator (Compose's `circularWavyProgressIndicator(progress,
    /// …)`): from 12 o'clock clockwise `progress()` of the ring in `color` (`stroke`), waving by
    /// `amplitude(progress)` (0..1) with about `wavelength` long waves moving `wave_speed` a second; the rest of a
    /// plain ring in `track_color` (`track_stroke`) `gap_size` away.
    fn circular_wavy_progress_indicator(
        self,
        progress: Rc<dyn Fn() -> f32>,
        color: Color,
        track_color: Color,
        stroke: Stroke,
        track_stroke: Stroke,
        gap_size: Dp,
        amplitude: Rc<dyn Fn(f32) -> f32>,
        wavelength: Dp,
        wave_speed: Dp,
    ) -> Modifier;

    /// Draws an indeterminate circular wavy progress indicator (Compose's `circularWavyProgressIndicator(color,
    /// …, amplitude: Float, …)`): an arc growing and shrinking as it turns, waving by `amplitude` (0..1).
    fn indeterminate_circular_wavy_progress_indicator(
        self,
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

impl CircularWavyProgressModifiers for Modifier {
    fn circular_wavy_progress_indicator(
        self,
        progress: Rc<dyn Fn() -> f32>,
        color: Color,
        track_color: Color,
        stroke: Stroke,
        track_stroke: Stroke,
        gap_size: Dp,
        amplitude: Rc<dyn Fn(f32) -> f32>,
        wavelength: Dp,
        wave_speed: Dp,
    ) -> Modifier {
        self.then(Modifier::element(DeterminateCircularWavyProgressElement {
            progress,
            amplitude,
            base: BaseCircularWavyProgressElement {
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

    fn indeterminate_circular_wavy_progress_indicator(
        self,
        color: Color,
        track_color: Color,
        stroke: Stroke,
        track_stroke: Stroke,
        gap_size: Dp,
        amplitude: f32,
        wavelength: Dp,
        wave_speed: Dp,
    ) -> Modifier {
        self.then(Modifier::element(IndeterminateCircularWavyProgressElement {
            amplitude,
            base: BaseCircularWavyProgressElement {
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

/// What both elements hold (Compose's abstract `BaseCircularWavyProgressElement`): equal by these.
#[derive(Clone, PartialEq)]
struct BaseCircularWavyProgressElement {
    color: Color,
    track_color: Color,
    stroke: Stroke,
    track_stroke: Stroke,
    gap_size: Dp,
    wavelength: Dp,
    wave_speed: Dp,
}

impl BaseCircularWavyProgressElement {
    fn update(&self, node: &Rc<BaseCircularWavyProgressNode>) {
        node.set_color(self.color);
        node.set_track_color(self.track_color);
        node.set_stroke(self.stroke.clone());
        node.set_track_stroke(self.track_stroke.clone());
        node.set_gap_size(self.gap_size);
        node.set_wavelength(self.wavelength);
        node.set_wave_speed(self.wave_speed);
    }

    fn create_base(&self, kind: WaveKind) -> Rc<BaseCircularWavyProgressNode> {
        Rc::new(BaseCircularWavyProgressNode {
            color: Cell::new(self.color),
            track_color: Cell::new(self.track_color),
            stroke: RefCell::new(self.stroke.clone()),
            track_stroke: RefCell::new(self.track_stroke.clone()),
            gap_size: Cell::new(self.gap_size),
            wavelength: Cell::new(self.wavelength),
            wave_speed: Cell::new(self.wave_speed),
            circular_shapes: RefCell::new(CircularShapes::new()),
            progress_drawing_cache: RefCell::new(CircularProgressDrawingCache::new()),
            wave_offset_state: mutable_float_state_of(0.0),
            offset_animatable: RefCell::new(None),
            offset_animation_job: RefCell::new(None),
            vertex_count_for_current_animation: Cell::new(-1),
            kind,
            handle: RefCell::new(None),
            cache_draw_node: RefCell::new(None),
        })
    }
}

/// How a node tells whether it draws a wave (Compose's abstract `isDrawingWave`): the determinate node by its
/// animated amplitude, the indeterminate one by its amplitude.
enum WaveKind {
    Determinate { amplitude_state: MutableState<f32> },
    Indeterminate { amplitude: Cell<f32> },
}

/// What both nodes share with their draw caches (Compose's abstract `BaseCircularWavyProgressNode`, a
/// DelegatingNode): its parameters (a change of color drawn again, of a size the cache built again, of the wave
/// the offset animation started again), the shapes, the drawing cache, the wave offset animation. Its draw cache
/// node is delegated to as it is first attached (a node has its context then).
struct BaseCircularWavyProgressNode {
    color: Cell<Color>,
    track_color: Cell<Color>,
    stroke: RefCell<Stroke>,
    track_stroke: RefCell<Stroke>,
    gap_size: Cell<Dp>,
    wavelength: Cell<Dp>,
    wave_speed: Cell<Dp>,
    circular_shapes: RefCell<CircularShapes>,
    progress_drawing_cache: RefCell<CircularProgressDrawingCache>,
    // Offset animation
    wave_offset_state: MutableState<f32>,
    offset_animatable: RefCell<Option<Animatable<f32, AnimationVector1D>>>,
    offset_animation_job: RefCell<Option<Job>>,
    // A vertex count used to calculate the speed of the wave offset animation. The animation will
    // only run when this number is greater than zero.
    vertex_count_for_current_animation: Cell<i32>,
    kind: WaveKind,
    handle: RefCell<Option<NodeHandle>>,
    cache_draw_node: RefCell<Option<DelegatedNode<CacheDrawModifierNode>>>,
}

impl BaseCircularWavyProgressNode {
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
            // Wavelength affects the offset animation
            self.start_offset_animation();
        }
    }

    fn set_wave_speed(self: &Rc<Self>, value: Dp) {
        if self.wave_speed.get() != value {
            self.wave_speed.set(value);
            // Speed affects the offset animation
            self.start_offset_animation();
        }
    }

    fn is_attached(&self) -> bool {
        self.handle.borrow().as_ref().is_some_and(NodeHandle::is_attached)
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

    /// Whether it draws a wave now (Compose's `isDrawingWave`).
    fn is_drawing_wave(&self) -> bool {
        match &self.kind {
            WaveKind::Determinate { amplitude_state } => amplitude_state.value() > 0.0,
            WaveKind::Indeterminate { amplitude } => amplitude.get() > 0.0,
        }
    }

    /// Compose's `trackPathProvider`: the plain ring's path.
    fn track_path_provider(&self, path: &mut Path) -> bool {
        self.circular_shapes.borrow().get_track_path(path);
        true
    }

    /// Compose's `progressPathProvider`: the progress's shape at the amplitude, twice round if it moves.
    fn progress_path_provider(&self, amplitude: f32, support_motion: bool, path: &mut Path) {
        self.circular_shapes.borrow().get_progress_path(amplitude, path, support_motion, 0.5, 0.5);
    }

    /// Starts the wave offset animation (Compose's `startOffsetAnimation`): attached and drawing a wave, with a
    /// speed, a wavelength and the vertex count known — a round of the offset every `wavelength / wave_speed`
    /// seconds a vertex (at least 50 ms), from where it is, forever; else the offset snapped to 0.
    fn start_offset_animation(self: &Rc<Self>) {
        // Cancel previous job if running
        self.stop_offset_animation();

        // Don't animate if not attached or not drawing a wave (e.g. zero amplitude).
        let scope = self.handle.borrow().as_ref().and_then(NodeHandle::coroutine_scope);
        let Some(scope) = scope.filter(|s| self.is_attached() && s.is_active()) else { return };
        if !self.is_drawing_wave() {
            return;
        }

        // Start the animation only if we have a valid vertex count and positive waveSpeed and
        // wavelength.
        let (wave_speed, wavelength) = (self.wave_speed.get(), self.wavelength.get());
        let vertex_count = self.vertex_count_for_current_animation.get();
        if wave_speed > Dp(0.0) && wavelength > Dp(0.0) && vertex_count > 0 {
            // Use the current vertex count from shapes if needed for duration precision.
            let duration_millis = fast_round_to_int((wavelength.0 / wave_speed.0) * 1000.0 * vertex_count as f32)
                .max(MinAnimationDuration);

            // Re-create animatable to ensure it picks up the latest offset state
            let start_offset = self.wave_offset_state.peek();
            let animatable: Animatable<f32, AnimationVector1D> = Animatable(start_offset);
            *self.offset_animatable.borrow_mut() = Some(animatable.clone());
            let wave_offset_state = self.wave_offset_state.clone();
            let job = named_args!(scope.launch(async move {
                // Update bounds relative to the current offset
                named_args!(animatable.update_bounds(Some(start_offset), Some(start_offset + 1.0)));
                let animation: TweenSpec<f32> =
                    named_args!(tween(duration_millis = duration_millis, easing = Arc::new(LinearEasing)));
                let spec: Arc<dyn AnimationSpec<f32>> =
                    Arc::new(InfiniteRepeatableSpec::new(animation, RepeatMode::Restart, StartOffset::default()));
                named_args!(animatable.animate_to(
                    start_offset + 1.0,
                    animation_spec = spec,
                    block = on_frame(move |a: &Animatable<f32, AnimationVector1D>| {
                        // Update the state on each frame
                        wave_offset_state.set(a.value() % 1.0)
                    })
                ))
                .await;
            }));
            *self.offset_animation_job.borrow_mut() = Some(job);
        } else {
            // No speed or wavelength, snap offset to 0
            self.wave_offset_state.set(0.0);
        }
    }

    /// Compose's `stopOffsetAnimation`.
    fn stop_offset_animation(&self) {
        if let Some(job) = self.offset_animation_job.borrow_mut().take() {
            job.cancel();
        }
        *self.offset_animatable.borrow_mut() = None;
    }

    /// Whether no offset animation runs (Compose's `offsetAnimationJob == null || offsetAnimationJob?.isCompleted
    /// == true`).
    fn offset_animation_done(&self) -> bool {
        self.offset_animation_job.borrow().as_ref().is_none_or(Job::is_completed)
    }

    /// Updates the shapes (Compose's `circularShapes.update`) and then the vertex count the offset animation
    /// runs by.
    fn update_shapes(&self, size: Size, wavelength_px: f32, requires_morph: bool) {
        let stroke_width = self.stroke.borrow().width;
        let vertex_count = {
            let mut shapes = self.circular_shapes.borrow_mut();
            shapes.update(size, wavelength_px, stroke_width, requires_morph);
            shapes.current_vertex_count.value()
        };
        // Update the vertexCountForCurrentAnimation after the circularShapes.update().
        if self.vertex_count_for_current_animation.get() != vertex_count {
            self.vertex_count_for_current_animation.set(vertex_count.max(MinCircularVertexCount));
        }
    }

    /// Brings the drawing cache up to date and draws the ring (the body of both nodes' `onDrawWithContent`).
    #[allow(clippy::too_many_arguments)]
    fn update_and_draw(
        &self,
        d: &mut DrawScope,
        enable_motion: bool,
        end_progress: f32,
        amplitude: f32,
        wave_offset: f32,
        wavelength_px: f32,
        gap_px: f32,
    ) {
        let (stroke, track_stroke) = (self.stroke.borrow().clone(), self.track_stroke.borrow().clone());
        let mut cache = self.progress_drawing_cache.borrow_mut();
        cache.update_paths(
            d.size(),
            &|amplitude: f32, support_motion: bool, path: &mut Path| {
                self.progress_path_provider(amplitude, support_motion, path)
            },
            &|path: &mut Path| self.track_path_provider(path),
            enable_motion,
            0.0,
            end_progress,
            amplitude,
            wave_offset,
            wavelength_px,
            gap_px,
            &stroke,
            &track_stroke,
        );
        draw_circular_indicator(d, self.color.get(), self.track_color.get(), &stroke, &track_stroke, &cache);
    }

    /// Attaches the node: its handle kept, its draw cache delegated to the first time.
    fn attach(&self, cx: &NodeCx, cache: impl FnOnce() -> CacheDrawModifierNode) {
        *self.handle.borrow_mut() = Some(cx.handle());
        if self.cache_draw_node.borrow().is_none() {
            let delegated = cx.delegate(cache());
            *self.cache_draw_node.borrow_mut() = Some(delegated);
        }
    }
}

// ---- The determinate element and node ----------------------------------------------------------------------------

/// Compose's private `DeterminateCircularWavyProgressElement`: equal by its parameters, the lambdas by identity.
struct DeterminateCircularWavyProgressElement {
    progress: Rc<dyn Fn() -> f32>,
    amplitude: Rc<dyn Fn(f32) -> f32>,
    base: BaseCircularWavyProgressElement,
}

impl PartialEq for DeterminateCircularWavyProgressElement {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
            && Rc::ptr_eq(&self.progress, &other.progress)
            && Rc::ptr_eq(&self.amplitude, &other.amplitude)
    }
}

impl ModifierNodeElement for DeterminateCircularWavyProgressElement {
    type Node = DeterminateCircularWavyProgressNode;

    fn create(&self) -> DeterminateCircularWavyProgressNode {
        DeterminateCircularWavyProgressNode {
            base: self.base.create_base(WaveKind::Determinate { amplitude_state: mutable_float_state_of(0.0) }),
            own: Rc::new(DeterminateCircularOwn {
                progress: RefCell::new(self.progress.clone()),
                amplitude: RefCell::new(self.amplitude.clone()),
                amplitude_animatable: RefCell::new(None),
                amplitude_animation_job: RefCell::new(None),
            }),
        }
    }

    fn update(&self, node: &mut DeterminateCircularWavyProgressNode) {
        self.base.update(&node.base);
        let changed = !Rc::ptr_eq(&node.own.progress.borrow(), &self.progress)
            || !Rc::ptr_eq(&node.own.amplitude.borrow(), &self.amplitude);
        if changed {
            *node.own.progress.borrow_mut() = self.progress.clone();
            *node.own.amplitude.borrow_mut() = self.amplitude.clone();
            node.base.invalidate_draw_cache();
        }
    }

    fn name(&self) -> &'static str {
        "determinateCircularWavyProgressIndicator"
    }
}

/// The determinate node's own parameters and amplitude animation, shared with its draw cache.
struct DeterminateCircularOwn {
    progress: RefCell<Rc<dyn Fn() -> f32>>,
    amplitude: RefCell<Rc<dyn Fn(f32) -> f32>>,
    // Amplitude animation
    amplitude_animatable: RefCell<Option<Animatable<f32, AnimationVector1D>>>,
    amplitude_animation_job: RefCell<Option<Job>>,
}

/// Compose's private `DeterminateCircularWavyProgressNode`.
struct DeterminateCircularWavyProgressNode {
    base: Rc<BaseCircularWavyProgressNode>,
    own: Rc<DeterminateCircularOwn>,
}

impl DeterminateCircularWavyProgressNode {
    /// Compose's `cacheDrawNode`: the progress and the amplitude's target read (the amplitude animated to it,
    /// the wave offset animation started as it turns positive), the shapes updated; then each frame the paths
    /// brought up to date and drawn.
    fn cache_draw_node(&self) -> CacheDrawModifierNode {
        let (base, own) = (self.base.clone(), self.own.clone());
        CacheDrawModifierNode(move |scope: &mut CacheDrawScope| {
            let progress = own.progress.borrow().clone();
            let amplitude = own.amplitude.borrow().clone();
            let current_progress = fast_coerce_in(progress(), 0.0, 1.0);
            let current_gap_px = scope.to_px(base.gap_size.get());
            let current_wavelength_px = scope.to_px(base.wavelength.get());
            let enable_motion = base.wave_speed.get() > Dp(0.0);
            let WaveKind::Determinate { amplitude_state } = &base.kind else { unreachable!() };

            //  Amplitude animation
            let target_amplitude = fast_coerce_in(amplitude(current_progress), 0.0, 1.0);

            let animatable = own
                .amplitude_animatable
                .borrow_mut()
                .get_or_insert_with(|| {
                    amplitude_state.set(target_amplitude);
                    Animatable(target_amplitude)
                })
                .clone();

            // Launch amplitude animation if target changed and node attached/active
            let job_done = own.amplitude_animation_job.borrow().as_ref().is_none_or(Job::is_completed);
            if base.is_attached()
                && animatable.target_value() != target_amplitude
                && job_done
                && let Some(coroutine_scope) = base.handle.borrow().as_ref().and_then(NodeHandle::coroutine_scope)
            {
                let (state, node) = (amplitude_state.clone(), base.clone());
                let job = named_args!(coroutine_scope.launch(async move {
                    let spec: Arc<dyn AnimationSpec<f32>> = if animatable.peek() < target_amplitude {
                        Arc::new(IncreasingAmplitudeAnimationSpec())
                    } else {
                        Arc::new(DecreasingAmplitudeAnimationSpec())
                    };
                    named_args!(animatable.animate_to(
                        target_amplitude,
                        animation_spec = spec,
                        block = on_frame(move |a: &Animatable<f32, AnimationVector1D>| state.set(a.value()))
                    ))
                    .await;

                    // Check if we need to stop the animation if the target amplitude is 0
                    // at the end of the animation.
                    if target_amplitude == 0.0 {
                        node.stop_offset_animation();
                    }
                }));
                *own.amplitude_animation_job.borrow_mut() = Some(job);
            }

            // Pass stroke width for potentially calculating polygon radius accurately
            let requires_morph = own.amplitude_animation_job.borrow().is_some();
            base.update_shapes(scope.size(), current_wavelength_px, requires_morph);

            // Start the offset animation when the amplitude changes to a positive number. The
            // animation will end when the amplitudeAnimatable finishes and reached
            // zero amplitude again.
            if target_amplitude > 0.0 && base.offset_animation_done() {
                base.start_offset_animation();
            }

            let (base, amplitude_state) = (base.clone(), amplitude_state.clone());
            scope.on_draw_with_content(move |c| {
                // Read from the state here and update the CircularProgressDrawingCache inside
                // the onDrawWithContent to avoid voiding the cache at this
                // CacheDrawModifierNode.
                let animated_amplitude_value = amplitude_state.value();
                let wave_offset =
                    if animated_amplitude_value > 0.0 && enable_motion { base.wave_offset_state.value() } else { 0.0 };
                base.update_and_draw(
                    c,
                    enable_motion,
                    current_progress,
                    animated_amplitude_value,
                    wave_offset,
                    current_wavelength_px,
                    current_gap_px,
                );
            })
        })
    }
}

impl ModifierNode for DeterminateCircularWavyProgressNode {
    node_capabilities!();

    fn on_attach(&mut self, cx: &NodeCx) {
        let cache = || self.cache_draw_node();
        self.base.attach(cx, cache);
        // No op here. The first draw will resolve the circularShapes.currentVertexCount and will
        // trigger the updateOffsetAnimationState() to start the animation at the right speed.
    }

    fn on_detach(&mut self, _: &NodeCx) {
        *self.own.amplitude_animatable.borrow_mut() = None;
        self.base.vertex_count_for_current_animation.set(-1);
    }
}

// ---- The indeterminate element and node --------------------------------------------------------------------------

/// Compose's private `IndeterminateCircularWavyProgressElement`: equal by its parameters.
#[derive(PartialEq)]
struct IndeterminateCircularWavyProgressElement {
    amplitude: f32,
    base: BaseCircularWavyProgressElement,
}

impl ModifierNodeElement for IndeterminateCircularWavyProgressElement {
    type Node = IndeterminateCircularWavyProgressNode;

    fn create(&self) -> IndeterminateCircularWavyProgressNode {
        let amplitude = Cell::new(self.amplitude.clamp(0.0, 1.0));
        IndeterminateCircularWavyProgressNode {
            base: self.base.create_base(WaveKind::Indeterminate { amplitude }),
            own: Rc::new(IndeterminateCircularOwn {
                global_rotation_animatable: RefCell::new(None),
                additional_rotation_animatable: RefCell::new(None),
                progress_sweep_animatable: RefCell::new(None),
                indeterminate_animations_job: RefCell::new(None),
            }),
        }
    }

    fn update(&self, node: &mut IndeterminateCircularWavyProgressNode) {
        self.base.update(&node.base);
        node.set_amplitude(self.amplitude);
    }

    fn name(&self) -> &'static str {
        "indeterminateCircularWavyProgressIndicator"
    }
}

/// The indeterminate node's own animations, shared with its draw cache.
struct IndeterminateCircularOwn {
    // Specific animations for indeterminate progress
    global_rotation_animatable: RefCell<Option<Animatable<f32, AnimationVector1D>>>,
    additional_rotation_animatable: RefCell<Option<Animatable<f32, AnimationVector1D>>>,
    progress_sweep_animatable: RefCell<Option<Animatable<f32, AnimationVector1D>>>,
    indeterminate_animations_job: RefCell<Option<Job>>,
}

/// Compose's private `IndeterminateCircularWavyProgressNode`.
struct IndeterminateCircularWavyProgressNode {
    base: Rc<BaseCircularWavyProgressNode>,
    own: Rc<IndeterminateCircularOwn>,
}

impl IndeterminateCircularWavyProgressNode {
    fn amplitude_cell(&self) -> &Cell<f32> {
        let WaveKind::Indeterminate { amplitude } = &self.base.kind else { unreachable!() };
        amplitude
    }

    /// Compose's `amplitude` setter: kept in 0..1; turning positive the offset animation starts, turning 0 it
    /// stops.
    fn set_amplitude(&self, value: f32) {
        let coerced = value.clamp(0.0, 1.0);
        let field = self.amplitude_cell();
        let old_value = field.get();
        if old_value != coerced {
            field.set(coerced);
            // If amplitude > 0 was false and is now true, start offset animation.
            // If amplitude becomes 0, stop offset animation.
            if coerced > 0.0 && old_value == 0.0 {
                self.base.start_offset_animation()
            } else if coerced == 0.0 {
                self.base.stop_offset_animation()
            }
            self.base.invalidate_draw_cache();
        }
    }

    /// Starts the three endless animations (Compose's `startIndeterminateAnimations`): the global rotation (1080°
    /// in 6 s), the additional one (90° in 300 ms every 1.5 s) and the sweep (from 10% to 87% and back in 6 s),
    /// each from where it is, drawing again each frame.
    fn start_indeterminate_animations(&self) {
        if let Some(job) = self.own.indeterminate_animations_job.borrow_mut().take() {
            job.cancel();
        }

        let base = &self.base;
        let scope = base.handle.borrow().as_ref().and_then(NodeHandle::coroutine_scope);
        let Some(scope) = scope.filter(|s| base.is_attached() && s.is_active()) else { return };

        // Initialize the animatables if they are null
        let own = &self.own;
        let global = own.global_rotation_animatable.borrow_mut().get_or_insert_with(|| Animatable(0.0)).clone();
        let additional = own.additional_rotation_animatable.borrow_mut().get_or_insert_with(|| Animatable(0.0)).clone();
        let sweep = own
            .progress_sweep_animatable
            .borrow_mut()
            .get_or_insert_with(|| Animatable(CircularIndeterminateMinProgress))
            .clone();

        let inner = scope.clone();
        let handle = base.handle.borrow().clone();
        let job = named_args!(scope.launch(async move {
            // Launch all three indeterminate animations concurrently
            let redraw = move || {
                let handle = handle.clone();
                on_frame(move |_: &Animatable<f32, AnimationVector1D>| {
                    if let Some(h) = &handle {
                        h.invalidate_draw();
                    }
                })
            };
            let block = redraw();
            named_args!(inner.launch(async move {
                let spec: Arc<dyn AnimationSpec<f32>> =
                    Arc::new(circular_indeterminate_global_rotation_animation_spec());
                let target = global.peek() + CircularGlobalRotationDegreesTarget;
                named_args!(global.animate_to(target, animation_spec = spec, block = block)).await;
            }));
            let block = redraw();
            named_args!(inner.launch(async move {
                let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(circular_indeterminate_rotation_animation_spec());
                let target = additional.peek() + CircularAdditionalRotationDegreesTarget;
                named_args!(additional.animate_to(target, animation_spec = spec, block = block)).await;
            }));
            let block = redraw();
            named_args!(inner.launch(async move {
                let spec: Arc<dyn AnimationSpec<f32>> = Arc::new(circular_indeterminate_progress_animation_spec());
                let target =
                    if sweep.peek() < (CircularIndeterminateMinProgress + CircularIndeterminateMaxProgress) / 2.0 {
                        CircularIndeterminateMaxProgress
                    } else {
                        CircularIndeterminateMinProgress
                    };
                named_args!(sweep.animate_to(target, animation_spec = spec, block = block)).await;
            }));
        }));
        *self.own.indeterminate_animations_job.borrow_mut() = Some(job);
    }

    /// Compose's `cacheDrawNode`: the shapes updated (a morph only for an amplitude between 0 and 1), the wave
    /// offset animation started for a positive amplitude; then each frame the paths brought up to date and drawn
    /// turned by the rotations (and a quarter more: from 12 o'clock).
    fn cache_draw_node(&self) -> CacheDrawModifierNode {
        let (base, own) = (self.base.clone(), self.own.clone());
        CacheDrawModifierNode(move |scope: &mut CacheDrawScope| {
            let WaveKind::Indeterminate { amplitude } = &base.kind else { unreachable!() };
            let amplitude = amplitude.get();
            let current_wavelength_px = scope.to_px(base.wavelength.get());
            let current_gap_px = scope.to_px(base.gap_size.get());
            let enable_motion = base.wave_speed.get() > Dp(0.0) && amplitude > 0.0;

            // Update the CircularShapes with the values. It will generate the RoundedPolygons
            // required for the progress and track. Since the amplitude is not animating in the
            // indeterminate indicator, the requiresMorph is set to true only when the amplitude
            // is between 0 and 1. In those cases we will need a Morph object to generate a Path
            // that represents that amplitude.
            base.update_shapes(scope.size(), current_wavelength_px, amplitude > 0.0 && amplitude < 1.0);

            // Start the offset animation when the amplitude changes to a positive number. The
            // animation will end when the amplitudeAnimatable finishes and reached
            // zero amplitude again.
            if amplitude > 0.0 && base.offset_animation_done() {
                base.start_offset_animation();
            }

            let (base, own) = (base.clone(), own.clone());
            scope.on_draw_with_content(move |c| {
                // Get the current animation values (or defaults if not yet running).
                // Read from the states here and update the CircularProgressDrawingCache inside
                // the onDrawWithContent to avoid voiding the cache at this
                // CacheDrawModifierNode.
                let value = |a: &RefCell<Option<Animatable<f32, AnimationVector1D>>>, default: f32| {
                    a.borrow().as_ref().map_or(default, |a| a.value())
                };
                let current_global_rotation = value(&own.global_rotation_animatable, 0.0);
                let current_additional_rotation = value(&own.additional_rotation_animatable, 0.0);
                let current_progress_sweep = value(&own.progress_sweep_animatable, CircularIndeterminateMinProgress);
                let wave_offset = if enable_motion { base.wave_offset_state.value() } else { 0.0 };

                // Draw
                let pivot = c.center();
                c.rotate_with(current_global_rotation + current_additional_rotation + 90.0, pivot, |c| {
                    // Update the drawing cache
                    base.update_and_draw(
                        c,
                        enable_motion,
                        current_progress_sweep,
                        amplitude,
                        wave_offset,
                        current_wavelength_px,
                        current_gap_px,
                    );
                });
            })
        })
    }
}

impl ModifierNode for IndeterminateCircularWavyProgressNode {
    node_capabilities!();

    fn on_attach(&mut self, cx: &NodeCx) {
        let cache = || self.cache_draw_node();
        self.base.attach(cx, cache);
        self.start_indeterminate_animations();
        // The offset animation will be triggered from cacheDrawNode or amplitude setter
    }

    fn on_detach(&mut self, _: &NodeCx) {
        *self.own.global_rotation_animatable.borrow_mut() = None;
        *self.own.additional_rotation_animatable.borrow_mut() = None;
        *self.own.progress_sweep_animatable.borrow_mut() = None;
        self.base.vertex_count_for_current_animation.set(-1);
    }
}

// ---- Drawing, the drawing cache and the shapes -------------------------------------------------------------------

/// Draws the track and the progress (Compose's private `DrawScope.drawCircularIndicator`): each unless its color is
/// transparent or unspecified.
fn draw_circular_indicator(
    d: &mut DrawScope,
    color: Color,
    track_color: Color,
    stroke: &Stroke,
    track_stroke: &Stroke,
    drawing_cache: &CircularProgressDrawingCache,
) {
    // Draw the track
    if track_color != Color::Transparent && track_color != Color::Unspecified {
        named_args!(d.draw_path(
            &drawing_cache.track_path_to_draw,
            track_color,
            style = DrawStyle::Stroke(track_stroke.clone())
        ));
    }

    // Draw the progress
    if color != Color::Transparent && color != Color::Unspecified {
        named_args!(d.draw_path(
            &drawing_cache.progress_path_to_draw,
            color,
            style = DrawStyle::Stroke(stroke.clone())
        ));
    }
}

/// The paths a circular wavy indicator draws, made again only as what they depend on changes (Compose's private
/// `CircularProgressDrawingCache`): the full progress shape and the full track ring scaled to the size and
/// measured; the progress part cut from it (shifted and turned back by the offset when it moves), the track the
/// rest of the ring past the gaps.
struct CircularProgressDrawingCache {
    current_amplitude: f32,
    current_wavelength: f32,
    current_size: Size,
    /// The progress's start, 0..1.
    current_start_progress: f32,
    /// The progress's end, 0..1.
    current_end_progress: f32,
    current_indicator_track_gap_size: f32,
    current_wave_offset: f32,
    current_stroke: Stroke,
    current_track_stroke: Stroke,

    progress_path_length: f32,
    track_path_length: f32,
    current_progress_motion_enabled: bool,

    // Reusable Matrices for processing paths
    scale_matrix: Matrix,
    transform_matrix: Matrix,

    /// The full progress shape (twice round when it moves).
    full_progress_path: Path,
    /// The full track ring.
    full_track_path: Path,
    /// The progress part to draw.
    progress_path_to_draw: Path,
    /// The track part to draw.
    track_path_to_draw: Path,
    progress_path_measure: PathMeasure,
    track_path_measure: PathMeasure,
    /// The room the caps need at each end.
    current_stroke_cap_width: f32,
}

/// What gives the progress path (Compose's `progressPathProvider` lambda type, the parameters its provider uses).
type ProgressPathProvider<'p> = &'p dyn Fn(f32, bool, &mut Path);

/// What gives the track path (Compose's `trackPathProvider`): whether there is one.
type TrackPathProvider<'p> = &'p dyn Fn(&mut Path) -> bool;

impl CircularProgressDrawingCache {
    fn new() -> CircularProgressDrawingCache {
        let current_stroke = Stroke::default();
        CircularProgressDrawingCache {
            current_amplitude: -1.0,
            current_wavelength: -1.0,
            current_size: Size::Unspecified,
            current_start_progress: 0.0,
            current_end_progress: 0.0,
            current_indicator_track_gap_size: 0.0,
            current_wave_offset: -1.0,
            current_track_stroke: current_stroke.clone(),
            current_stroke,
            progress_path_length: 0.0,
            track_path_length: 0.0,
            current_progress_motion_enabled: false,
            scale_matrix: Matrix::new(),
            transform_matrix: Matrix::new(),
            full_progress_path: Path::new(),
            full_track_path: Path::new(),
            progress_path_to_draw: Path::new(),
            track_path_to_draw: Path::new(),
            progress_path_measure: PathMeasure::new(),
            track_path_measure: PathMeasure::new(),
            current_stroke_cap_width: 0.0,
        }
    }

    /// Brings the paths up to date (Compose's `updatePaths`).
    #[allow(clippy::too_many_arguments)]
    fn update_paths(
        &mut self,
        size: Size,
        progress_path_provider: ProgressPathProvider,
        track_path_provider: TrackPathProvider,
        enable_progress_motion: bool,
        start_progress: f32,
        end_progress: f32,
        amplitude: f32,
        wave_offset: f32,
        wavelength: f32,
        gap_size: f32,
        stroke: &Stroke,
        track_stroke: &Stroke,
    ) {
        let paths_updates = self.update_full_paths(
            size,
            progress_path_provider,
            track_path_provider,
            enable_progress_motion,
            amplitude,
            wavelength,
            gap_size,
            stroke,
            track_stroke,
        );
        self.update_draw_paths(paths_updates, start_progress, end_progress, wave_offset);
    }

    /// Compose's private `updateFullPaths`: whether the full paths were made again.
    #[allow(clippy::too_many_arguments)]
    fn update_full_paths(
        &mut self,
        size: Size,
        progress_path_provider: ProgressPathProvider,
        track_path_provider: TrackPathProvider,
        enable_progress_motion: bool,
        amplitude: f32,
        wavelength: f32,
        gap_size: f32,
        stroke: &Stroke,
        track_stroke: &Stroke,
    ) -> bool {
        if self.current_size == size
            && self.current_amplitude == amplitude
            && self.current_wavelength == wavelength
            && self.current_stroke == *stroke
            && self.current_track_stroke == *track_stroke
            && self.current_indicator_track_gap_size == gap_size
            && self.current_progress_motion_enabled == enable_progress_motion
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

        self.scale_matrix.reset();
        self.scale_matrix.scale(width - stroke.width, height - stroke.width, 1.0);

        self.full_progress_path.rewind();

        // Note that we pass in the enableProgressMotion when generating the Path. This may
        // generate a path that is double in length to support offsetting the drawing, so we make
        // sure to adjust for it when storing the progressPathLength.
        progress_path_provider(amplitude, enable_progress_motion, &mut self.full_progress_path);
        process_path(&mut self.full_progress_path, size, &self.scale_matrix);
        self.progress_path_measure.set_path(&self.full_progress_path, true);
        self.progress_path_length = if enable_progress_motion {
            self.progress_path_measure.length() / 2.0
        } else {
            self.progress_path_measure.length()
        };

        self.full_track_path.rewind();
        let track_path_for_amplitude = track_path_provider(&mut self.full_track_path);
        if track_path_for_amplitude {
            process_path(&mut self.full_track_path, size, &self.scale_matrix);
            self.track_path_measure.set_path(&self.full_track_path, true);
            self.track_path_length = self.track_path_measure.length();
        } else {
            self.track_path_length = 0.0;
        }

        // Cache the full path attributes
        self.current_size = size;
        self.current_amplitude = amplitude;
        self.current_wavelength = wavelength;
        self.current_stroke = stroke.clone();
        self.current_track_stroke = track_stroke.clone();
        self.current_indicator_track_gap_size = gap_size;
        self.current_progress_motion_enabled = enable_progress_motion;

        true
    }

    /// Compose's private `updateDrawPaths`: the progress part cut (from where the offset shifted it, the shape
    /// turned back by as much), the track the rest of the ring past the gaps and the caps' room.
    fn update_draw_paths(&mut self, force_update: bool, start_progress: f32, end_progress: f32, wave_offset: f32) {
        assert!(!self.current_size.is_unspecified(), "updateDrawPaths was called before updateFullPaths");
        if !force_update
            && self.current_start_progress == start_progress
            && self.current_end_progress == end_progress
            && self.current_wave_offset == wave_offset
        {
            // No update required
            return;
        }

        self.track_path_to_draw.rewind();
        self.progress_path_to_draw.rewind();

        let p_start = start_progress * self.progress_path_length;
        let p_stop = end_progress * self.progress_path_length;

        let track_gap_size = p_stop.min(self.current_indicator_track_gap_size);
        let horizontal_insets = p_stop.min(self.current_stroke_cap_width);
        let track_spacing = horizontal_insets * 2.0 + track_gap_size;

        // Handle offsetting the path when motion is enabled. We assume that the provided path when
        // motion is enabled was repeated to allow us to call getSegment with offsets and then
        // rotate the progressPathToDraw in order to create a shifted path as the progress moves.
        if self.current_progress_motion_enabled {
            let coerced_wave_offset = fast_coerce_in(wave_offset, 0.0, 1.0);
            let start_stop_shift = coerced_wave_offset * self.progress_path_length;

            self.progress_path_measure.get_segment(
                p_start + start_stop_shift,
                p_stop + start_stop_shift,
                &mut self.progress_path_to_draw,
                true,
            );

            let offset_angle = coerced_wave_offset * 360.0 % 360.0;
            if offset_angle != 0.0 {
                let full_progress_bounds = self.full_progress_path.get_bounds();
                let center = full_progress_bounds.center();
                // Rotate the progress path to adjust for the shift.
                self.progress_path_to_draw.translate(Offset::new(-center.x, -center.y));
                self.transform_matrix.reset();
                self.transform_matrix.rotate_z(-offset_angle);
                self.progress_path_to_draw.transform(&self.transform_matrix);
                // Translate the path to align its center with the available size center.
                self.progress_path_to_draw.translate(Offset::new(center.x, center.y));
            }
        } else {
            // No motion, so just grab the segment for the start and stop.
            self.progress_path_measure.get_segment(p_start, p_stop, &mut self.progress_path_to_draw, true);
        }
        if self.track_path_length > 0.0 {
            let t_start = end_progress * self.track_path_length + track_spacing;
            let t_stop = self.track_path_length - track_spacing;
            self.track_path_measure.get_segment(t_start, t_stop, &mut self.track_path_to_draw, true);
        }

        // Cache
        self.current_start_progress = start_progress;
        self.current_end_progress = end_progress;
        self.current_wave_offset = wave_offset;
    }
}

/// Scales a unit path to the size (less the stroke) and centers it there (Compose's private `processPath`).
fn process_path(path: &mut Path, size: Size, scale_matrix: &Matrix) {
    path.transform(scale_matrix);
    let progress_path_bounds = path.get_bounds();
    // Translate the path to align its center with the available size center.
    path.translate(size.center() - progress_path_bounds.center());
}

/// The shapes of a circular wavy indicator (Compose's private `CircularShapes`): a circle (the track) and a star of
/// as many rounded points as the circle holds wavelengths (the progress at full amplitude), both normalized; a morph
/// between them for the amplitudes between.
struct CircularShapes {
    current_size: Option<Size>,
    current_wavelength: f32,
    /// A circle polygon of as many vertices as the star (the track, and the morph's start).
    track_polygon: Option<RoundedPolygon>,
    /// A star polygon (the progress at full amplitude, the morph's end).
    active_indicator_polygon: Option<RoundedPolygon>,
    /// The morph from the track to the active indicator, made when one is asked for.
    active_indicator_morph: Option<Morph>,
    /// The vertices the polygons have (a state: the offset animation's speed follows it).
    current_vertex_count: MutableState<i32>,
}

impl CircularShapes {
    fn new() -> CircularShapes {
        CircularShapes {
            current_size: None,
            current_wavelength: -1.0,
            track_polygon: None,
            active_indicator_polygon: None,
            active_indicator_morph: None,
            current_vertex_count: mutable_int_state_of(-1),
        }
    }

    /// Makes the polygons for `size` and `wavelength` (Compose's `update`): as many vertices as 2πr / wavelength
    /// (r to the stroke's middle), at least 5; the morph too if `requires_morph` (or later, the first time it does).
    fn update(&mut self, size: Size, wavelength: f32, stroke_width: f32, requires_morph: bool) {
        assert!(wavelength > 0.0, "Wavelength should be greater than zero");
        if Some(size) == self.current_size && wavelength == self.current_wavelength {
            if requires_morph && self.active_indicator_morph.is_none() {
                // This is the first time a requiredMorph is set, so we need to create a new Morph.
                // At this point, the trackPolygon and activeIndicatorPolygon are already set.
                self.active_indicator_morph = Some(Morph::new(
                    self.track_polygon.clone().expect("the track polygon"),
                    self.active_indicator_polygon.clone().expect("the active indicator polygon"),
                ));
            }
            return;
        }
        // Compute the number of edges as a factor of the circle size that the morph will be
        // rendered in and its proposed wavelength (2πr / wavelength), where the radius takes into
        // account the stroke's width.
        let r = size.min_dimension() / 2.0 - stroke_width / 2.0;
        let num_vertices = MinCircularVertexCount.max((2.0 * PI * r as f64 / wavelength as f64 + 0.5).floor() as i32);

        if num_vertices != self.current_vertex_count.peek() {
            // Note that we match the vertices number at the track's polygon. This will result in a
            // smoother morphing between the active indicator and the track.
            let track = RoundedPolygon::circle(num_vertices, 1.0, 0.0, 0.0).normalized();
            let active = RoundedPolygon::star(
                num_vertices,
                1.0,
                0.75,
                CornerRounding::new(0.35, 0.4),
                Some(CornerRounding::new(0.5, 0.0)),
                None,
                0.0,
                0.0,
            )
            .normalized();
            if requires_morph {
                self.active_indicator_morph = Some(Morph::new(track.clone(), active.clone()));
            }
            self.track_polygon = Some(track);
            self.active_indicator_polygon = Some(active);
        }

        self.current_size = Some(size);
        self.current_wavelength = wavelength;
        self.current_vertex_count.set(num_vertices);
    }

    /// The track's path into `path` (Compose's `getTrackPath`).
    fn get_track_path(&self, path: &mut Path) {
        if let Some(track) = &self.track_polygon {
            track.to_path_into(path, 270, false, true);
        }
    }

    /// The progress's path into `path` at `amplitude` (Compose's `getProgressPath`): the morph's there; without a
    /// morph the star at full amplitude, else the circle.
    fn get_progress_path(
        &self,
        amplitude: f32,
        path: &mut Path,
        repeat_path: bool,
        rotation_pivot_x: f32,
        rotation_pivot_y: f32,
    ) {
        if let Some(morph) = &self.active_indicator_morph {
            // The RoundedPolygon used in the Morph were normalized (i.e. moves to
            // (0.5, 0.5)).
            morph.to_path_into(amplitude, path, 270, repeat_path, true, rotation_pivot_x, rotation_pivot_y);
        } else {
            // In case there is no Morph (i.e. it was never created at the update call because a
            // requiresMorph was never true), we return the active indicator polygon in case the
            // amplitude is 1. Otherwise, we return the track polygon.
            match (&self.active_indicator_polygon, &self.track_polygon) {
                (Some(active), _) if amplitude == 1.0 => active.to_path_into(path, 270, repeat_path, true),
                (_, Some(track)) => track.to_path_into(path, 270, repeat_path, true),
                _ => {}
            }
        }
    }
}

const MinCircularVertexCount: i32 = 5;
// Minimum animation duration to ensure we don't overwhelm the CPU.
const MinAnimationDuration: i32 = 50;
