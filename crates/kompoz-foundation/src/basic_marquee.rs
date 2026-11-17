//! `basicMarquee`, as Compose's BasicMarquee.kt: content wider than its room scrolled across it, over and over, a
//! copy following it after a gap, at a set speed, waiting between passes, at once or while focused.

use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, LinearEasing, StartOffset, StartOffsetType, TweenSpec,
    infinite_repeatable, repeatable,
};
use kompoz_coroutines::{CoroutineScope, Job, snapshot_flow, with_context};
use kompoz_graphics::{Density, Dp, IntOffset, IntSize, LayoutDirection};
use kompoz_runtime::{DerivedState, MutableState, derived_state_of, mutable_state_of, named, named_args};
use kompoz_ui::draw::{ContentDrawScope, DrawModifierNode, GraphicsLayer};
use kompoz_ui::focus::{FocusEventModifierNode, FocusState};
use kompoz_ui::graphics_context::ClipOp;
use kompoz_ui::layout::{LayerBlock, LayoutModifierNode};
use kompoz_ui::motion_duration_scale::MotionDurationScale;
use kompoz_ui::platform::LocalPlatform;
use kompoz_ui::{
    Constraints, IntrinsicMeasurable, IntrinsicMeasureScope, Measurable, MeasureResult, MeasureScope, Modifier,
    ModifierNode, ModifierNodeElement, NodeCx, NodeHandle, node_capabilities,
};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

/// The defaults of [`basic_marquee`](MarqueeModifiers::basic_marquee) (Compose's MarqueeDefaults, Android's
/// TextView's numbers).
pub struct MarqueeDefaults;

impl MarqueeDefaults {
    /// How many times it scrolls by (Compose's `Iterations`).
    pub const Iterations: i32 = 3;
    /// How long it waits between passes, ms (Compose's `RepeatDelayMillis`).
    pub const RepeatDelayMillis: i32 = 1_200;
    /// The gap before the copy: a third of the room (Compose's `Spacing`).
    pub const Spacing: MarqueeSpacing = MarqueeSpacing { kind: SpacingKind::FractionOfContainer(1.0 / 3.0) };
    /// How fast it scrolls, per second (Compose's `Velocity`).
    pub const Velocity: Dp = Dp(30.0);
}

/// When a marquee scrolls (Compose's MarqueeAnimationMode).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MarqueeAnimationMode(i32);

impl MarqueeAnimationMode {
    /// As soon as it is shown (Compose's `Immediately`).
    pub const Immediately: MarqueeAnimationMode = MarqueeAnimationMode(0);
    /// While it (or something in it) has the focus (Compose's `WhileFocused`); it needs to be focusable.
    pub const WhileFocused: MarqueeAnimationMode = MarqueeAnimationMode(1);
}

impl fmt::Display for MarqueeAnimationMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MarqueeAnimationMode::Immediately => write!(f, "Immediately"),
            MarqueeAnimationMode::WhileFocused => write!(f, "WhileFocused"),
            _ => panic!("invalid value: {}", self.0),
        }
    }
}

impl fmt::Debug for MarqueeAnimationMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The gap between a marquee's content and its copy, px, from the density, the content's width and the room's
/// (Compose's `fun interface MarqueeSpacing`): a fixed one ([`MarqueeSpacing()`](fn@MarqueeSpacing)), a fraction of
/// the room ([`fraction_of_container`](MarqueeSpacing::fraction_of_container)), or one's own
/// ([`MarqueeSpacing::new`]). The first two compare by their values, one's own by its instance.
#[derive(Clone)]
pub struct MarqueeSpacing {
    kind: SpacingKind,
}

#[derive(Clone)]
enum SpacingKind {
    Fixed(Dp),
    FractionOfContainer(f32),
    Custom(Arc<dyn Fn(&Density, i32, i32) -> i32 + Send + Sync>),
}

impl PartialEq for MarqueeSpacing {
    fn eq(&self, other: &Self) -> bool {
        match (&self.kind, &other.kind) {
            (SpacingKind::Fixed(a), SpacingKind::Fixed(b)) => a == b,
            (SpacingKind::FractionOfContainer(a), SpacingKind::FractionOfContainer(b)) => a == b,
            (SpacingKind::Custom(a), SpacingKind::Custom(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl fmt::Debug for MarqueeSpacing {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            SpacingKind::Fixed(dp) => write!(f, "MarqueeSpacing({dp:?})"),
            SpacingKind::FractionOfContainer(fraction) => write!(f, "MarqueeSpacing.fractionOfContainer({fraction})"),
            SpacingKind::Custom(_) => write!(f, "MarqueeSpacing(custom)"),
        }
    }
}

impl MarqueeSpacing {
    /// A gap worked out by `calculate_spacing(density, content_width, container_width)` (Compose's `MarqueeSpacing
    /// { contentWidth, containerWidth -> … }`).
    pub fn new(calculate_spacing: impl Fn(&Density, i32, i32) -> i32 + Send + Sync + 'static) -> MarqueeSpacing {
        MarqueeSpacing { kind: SpacingKind::Custom(Arc::new(calculate_spacing)) }
    }

    /// A gap of `fraction` of the room, rounded (Compose's `MarqueeSpacing.fractionOfContainer`).
    pub fn fraction_of_container(fraction: f32) -> MarqueeSpacing {
        MarqueeSpacing { kind: SpacingKind::FractionOfContainer(fraction) }
    }

    /// The gap, px (Compose's `Density.calculateSpacing`).
    pub fn calculate_spacing(&self, density: &Density, content_width: i32, container_width: i32) -> i32 {
        match &self.kind {
            SpacingKind::Fixed(spacing) => density.round_to_px(*spacing),
            SpacingKind::FractionOfContainer(fraction) => (fraction * container_width as f32).round() as i32,
            SpacingKind::Custom(calculate) => calculate(density, content_width, container_width),
        }
    }
}

/// A gap of `spacing` (Compose's `MarqueeSpacing(spacing: Dp)`).
pub fn MarqueeSpacing(spacing: Dp) -> MarqueeSpacing {
    MarqueeSpacing { kind: SpacingKind::Fixed(spacing) }
}

/// The marquee modifier.
#[named(trait MarqueeModifiers)]
impl Modifier {
    /// Scrolls its content across its room when it is wider (Compose's `Modifier.basicMarquee`): measured as wide
    /// as it wants, laid out as wide as it may be and clipped, it scrolls by `velocity` a second (backwards when
    /// negative), a copy following it `spacing` behind, `iterations` times (`i32::MAX`: for ever), waiting
    /// `repeat_delay_millis` between passes and `initial_delay_millis` before the first (Compose's default, none
    /// given: `repeat_delay_millis` scrolling at once, 0 while focused); `animation_mode` says whether it scrolls at
    /// once or while focused (the layout must be focusable then; it starts again from the start each time). It
    /// scrolls by the frame clock, whatever the animator scale says.
    ///
    /// Kotlin: `basicMarquee(iterations: Int = MarqueeDefaults.Iterations, animationMode: MarqueeAnimationMode = MarqueeAnimationMode.Immediately, repeatDelayMillis: Int = MarqueeDefaults.RepeatDelayMillis, initialDelayMillis: Int? = null, spacing: MarqueeSpacing = MarqueeDefaults.Spacing, velocity: Dp = MarqueeDefaults.Velocity)`
    fn basic_marquee(
        self,
        #[default(MarqueeDefaults::Iterations)] iterations: i32,
        #[default(MarqueeAnimationMode::Immediately)] animation_mode: MarqueeAnimationMode,
        #[default(MarqueeDefaults::RepeatDelayMillis)] repeat_delay_millis: i32,
        #[default(None)] initial_delay_millis: Option<i32>,
        #[default(MarqueeDefaults::Spacing)] spacing: MarqueeSpacing,
        #[default(MarqueeDefaults::Velocity)] velocity: Dp,
    ) -> Modifier {
        // (Compose's default depends on the parameters before it, which a Rust default can't read)
        let initial_delay_millis = initial_delay_millis
            .unwrap_or(if animation_mode == MarqueeAnimationMode::Immediately { repeat_delay_millis } else { 0 });
        self.then_element(MarqueeModifierElement {
            iterations,
            animation_mode,
            delay_millis: repeat_delay_millis,
            initial_delay_millis,
            spacing,
            velocity,
        })
    }
}

#[derive(PartialEq)]
struct MarqueeModifierElement {
    iterations: i32,
    animation_mode: MarqueeAnimationMode,
    delay_millis: i32,
    initial_delay_millis: i32,
    spacing: MarqueeSpacing,
    velocity: Dp,
}

impl ModifierNodeElement for MarqueeModifierElement {
    type Node = MarqueeModifierNode;
    fn create(&self) -> MarqueeModifierNode {
        MarqueeModifierNode::new(
            self.iterations,
            self.animation_mode,
            self.delay_millis,
            self.initial_delay_millis,
            self.spacing.clone(),
            self.velocity,
        )
    }
    fn update(&self, node: &mut MarqueeModifierNode) {
        node.update(
            self.iterations,
            self.animation_mode,
            self.delay_millis,
            self.initial_delay_millis,
            self.spacing.clone(),
            self.velocity,
        );
    }
    fn name(&self) -> &'static str {
        "basicMarquee"
    }
}

/// What the node and its animation share: the widths measured, the focus, the spacing and mode (states, read by
/// the animation's snapshot flow and by drawing), the offset (Compose's MarqueeModifierNode's fields).
struct MarqueeShared {
    content_width: MutableState<i32>,
    container_width: MutableState<i32>,
    has_focus: MutableState<bool>,
    spacing: MutableState<MarqueeSpacing>,
    animation_mode: MutableState<MarqueeAnimationMode>,
    offset: Animatable<f32, AnimationVector1D>,
    spacing_px: DerivedState<i32>,
}

/// The node of `basic_marquee` (Compose's MarqueeModifierNode): it measures its content unbounded across, lays out
/// at what it may be, notes the focus, and draws the content (recorded in a layer) twice through a sliding clip
/// window; a coroutine animates the offset while the content doesn't fit.
struct MarqueeModifierNode {
    iterations: i32,
    delay_millis: i32,
    initial_delay_millis: i32,
    velocity: Dp,
    shared: Rc<MarqueeShared>,
    density: Rc<Cell<Density>>,
    animation_job: Option<Job>,
    marquee_layer: Option<GraphicsLayer>,
    /// The content's layer block (Compose's default one: kept, so placing again isn't a new layer).
    layer_block: LayerBlock,
    handle: Option<NodeHandle>,
}

impl MarqueeModifierNode {
    fn new(
        iterations: i32,
        animation_mode: MarqueeAnimationMode,
        delay_millis: i32,
        initial_delay_millis: i32,
        spacing: MarqueeSpacing,
        velocity: Dp,
    ) -> MarqueeModifierNode {
        let content_width = mutable_state_of(0);
        let container_width = mutable_state_of(0);
        let spacing = mutable_state_of(spacing);
        let density = Rc::new(Cell::new(Density::default()));
        let spacing_px = {
            let (spacing, content, container, density) =
                (spacing.clone(), content_width.clone(), container_width.clone(), density.clone());
            derived_state_of(move || spacing.get().calculate_spacing(&density.get(), content.get(), container.get()))
        };
        MarqueeModifierNode {
            iterations,
            delay_millis,
            initial_delay_millis,
            velocity,
            shared: Rc::new(MarqueeShared {
                content_width,
                container_width,
                has_focus: mutable_state_of(false),
                spacing,
                animation_mode: mutable_state_of(animation_mode),
                offset: Animatable(0.0),
                spacing_px,
            }),
            density,
            animation_job: None,
            marquee_layer: None,
            layer_block: Rc::new(|_| {}),
            handle: None,
        }
    }

    fn update(
        &mut self,
        iterations: i32,
        animation_mode: MarqueeAnimationMode,
        delay_millis: i32,
        initial_delay_millis: i32,
        spacing: MarqueeSpacing,
        velocity: Dp,
    ) {
        self.shared.spacing.set(spacing);
        self.shared.animation_mode.set(animation_mode);
        if self.iterations != iterations
            || self.delay_millis != delay_millis
            || self.initial_delay_millis != initial_delay_millis
            || self.velocity != velocity
        {
            self.iterations = iterations;
            self.delay_millis = delay_millis;
            self.initial_delay_millis = initial_delay_millis;
            self.velocity = velocity;
            self.restart_animation();
        }
    }

    /// The animation started again, once the one before is cancelled (Compose's `restartAnimation`).
    fn restart_animation(&mut self) {
        let old_job = self.animation_job.take();
        if let Some(old) = &old_job {
            old.cancel();
        }
        let Some(scope) = self.handle.as_ref().filter(|h| h.is_attached()).and_then(|h| h.coroutine_scope()) else {
            return;
        };
        let (shared, density) = (self.shared.clone(), self.density.clone());
        let (iterations, initial_delay_millis, delay_millis, velocity) =
            (self.iterations, self.initial_delay_millis, self.delay_millis, self.velocity);
        let node_scope = scope.clone();
        self.animation_job = Some(scope.launch(async move {
            // (the cancellation of the one before waited for)
            if let Some(old) = old_job {
                old.join().await;
            }
            run_animation(shared, density, node_scope, iterations, initial_delay_millis, delay_millis, velocity).await;
        }));
    }
}

/// While the content doesn't fit (and is focused, in `WhileFocused`), animates the offset over a pass of the
/// content and its gap, as many times as asked, from the start each time it starts again (Compose's
/// `runAnimation`).
async fn run_animation(
    shared: Rc<MarqueeShared>,
    density: Rc<Cell<Density>>,
    node_scope: CoroutineScope,
    iterations: i32,
    initial_delay_millis: i32,
    delay_millis: i32,
    velocity: Dp,
) {
    if iterations <= 0 {
        // (no animation)
        return;
    }
    // Not slowed or sped up by the animator scale (Compose's FixedMotionDurationScale).
    with_context(MotionDurationScale::new(1.0), async move {
        let s = shared.clone();
        let flow = snapshot_flow(move || {
            // (nothing to animate when the content fits)
            if s.content_width.get() <= s.container_width.get() {
                return None;
            }
            if s.animation_mode.get() == MarqueeAnimationMode::WhileFocused && !s.has_focus.get() {
                return None;
            }
            Some((s.content_width.get() + s.spacing_px.get()) as f32)
        });
        flow.collect_latest(move |content_with_spacing_width: Option<f32>| {
            let (offset, density, node_scope) = (shared.offset.clone(), density.get(), node_scope.clone());
            async move {
                let Some(content_with_spacing_width) = content_with_spacing_width else { return };
                let spec = create_marquee_animation_spec(
                    iterations,
                    content_with_spacing_width,
                    initial_delay_millis,
                    delay_millis,
                    velocity,
                    &density,
                );
                offset.snap_to(0.0).await;
                // (Kotlin's `finally`: back at the start when the animation ends, or is cancelled, the focus lost)
                let reset = SnapBackOnDrop { offset: offset.clone(), scope: node_scope };
                named_args!(offset.animate_to(content_with_spacing_width, animation_spec = spec)).await;
                drop(reset);
            }
        })
        .await;
    })
    .await;
}

/// Snaps the offset back to the start as it goes (Kotlin's `finally { offset.snapTo(0f) }`, also when the
/// animation's coroutine is cancelled where it waits).
struct SnapBackOnDrop {
    offset: Animatable<f32, AnimationVector1D>,
    scope: CoroutineScope,
}

impl Drop for SnapBackOnDrop {
    fn drop(&mut self) {
        self.scope.launch(self.offset.snap_to(0.0));
    }
}

/// A pass at `velocity` over `target_value` px, `delay_millis` before each but the first's
/// `initial_delay_millis`, repeated `iterations` times (Compose's `createMarqueeAnimationSpec`).
fn create_marquee_animation_spec(
    iterations: i32,
    target_value: f32,
    initial_delay_millis: i32,
    delay_millis: i32,
    velocity: Dp,
    density: &Density,
) -> Arc<dyn AnimationSpec<f32>> {
    let px_per_sec = density.to_px(velocity);
    let single_spec = velocity_based_tween(px_per_sec.abs(), target_value, delay_millis);
    // (the first pass's delay is the initial one: the repeated delay taken off)
    let start_offset = StartOffset::new(-delay_millis + initial_delay_millis, StartOffsetType::Delay);
    if iterations == i32::MAX {
        Arc::new(named_args!(infinite_repeatable(single_spec, initial_start_offset = start_offset)))
    } else {
        Arc::new(named_args!(repeatable(iterations, single_spec, initial_start_offset = start_offset)))
    }
}

/// A linear tween over `target_value` at `velocity` px a second (Compose's `velocityBasedTween`).
fn velocity_based_tween(velocity: f32, target_value: f32, delay_millis: i32) -> TweenSpec<f32> {
    let px_per_milli = velocity / 1000.0;
    TweenSpec::new((target_value / px_per_milli).ceil() as i32, delay_millis, LinearEasing)
}

impl ModifierNode for MarqueeModifierNode {
    node_capabilities!(layout, draw, focus_event);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
        self.density.set(cx.density());
        if let Some(layer) = self.marquee_layer.take() {
            // (shouldn't happen: detached in between)
            layer.release();
        }
        self.marquee_layer = Some(GraphicsLayer::new(Some(cx.current_value_of(&LocalPlatform))));
        self.restart_animation();
    }

    fn on_detach(&mut self, _cx: &NodeCx) {
        if let Some(job) = self.animation_job.take() {
            job.cancel();
        }
        if let Some(layer) = self.marquee_layer.take() {
            layer.release();
        }
    }
}

impl FocusEventModifierNode for MarqueeModifierNode {
    fn on_focus_event(&mut self, state: FocusState) {
        self.shared.has_focus.set(state.has_focus());
    }
}

impl LayoutModifierNode for MarqueeModifierNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        self.density.set(scope.density());
        let child_constraints = Constraints::new(
            constraints.min_width,
            Constraints::Infinity,
            constraints.min_height,
            constraints.max_height,
        );
        let placeable = measurable.measure(child_constraints);
        let container_width = constraints.constrain_width(placeable.width());
        self.shared.container_width.set(container_width);
        self.shared.content_width.set(placeable.width());
        let block = self.layer_block.clone();
        scope.layout(container_width, placeable.height(), move |p| {
            // (in a layer: an animation frame doesn't draw the parent again)
            p.place_with_layer_block(&placeable, IntOffset::Zero, block.clone(), 0.0);
        })
    }

    // (the intrinsics don't set the widths: Compose's b/278729564)
    fn min_intrinsic_width(&mut self, _: &IntrinsicMeasureScope, _: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        0
    }
    fn max_intrinsic_width(
        &mut self,
        _: &IntrinsicMeasureScope,
        measurable: &dyn IntrinsicMeasurable,
        height: i32,
    ) -> i32 {
        measurable.max_intrinsic_width(height)
    }
    fn min_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, measurable: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        measurable.min_intrinsic_height(Constraints::Infinity)
    }
    fn max_intrinsic_height(&mut self, _: &IntrinsicMeasureScope, measurable: &dyn IntrinsicMeasurable, _: i32) -> i32 {
        measurable.max_intrinsic_height(Constraints::Infinity)
    }
}

impl DrawModifierNode for MarqueeModifierNode {
    fn draw(&mut self, scope: &mut ContentDrawScope) {
        // The first copy drawn in [0, content width], the second in [content width + spacing, twice that]; the
        // clip window, as wide as the room, slides over them the way the velocity and the layout direction say.
        let shared = &self.shared;
        let offset = shared.offset.value();
        let content_width = shared.content_width.get();
        let container_width = shared.container_width.get();
        let spacing_px = shared.spacing_px.get();
        let clip_window_offset = if self.velocity > Dp(0.0) {
            match scope.layout_direction() {
                LayoutDirection::Ltr => offset,
                LayoutDirection::Rtl => -offset + (content_width * 2 + spacing_px - container_width) as f32,
            }
        } else {
            match scope.layout_direction() {
                LayoutDirection::Ltr => -offset + (content_width + spacing_px) as f32,
                LayoutDirection::Rtl => offset + (content_width - container_width) as f32,
            }
        };
        let first_copy_visible = clip_window_offset < content_width as f32;
        let second_copy_visible = clip_window_offset + container_width as f32 > (content_width + spacing_px) as f32;
        let second_copy_offset = (content_width + spacing_px) as f32;
        let draw_height = scope.size().height;
        if let Some(layer) = &self.marquee_layer {
            scope.record_with(layer, IntSize::new(content_width, draw_height.round() as i32), |s| s.draw_content());
        }
        let layer = self.marquee_layer.clone();
        scope.clip_rect_with(0.0, 0.0, container_width as f32, draw_height, ClipOp::Intersect, |s| {
            s.translate_with(-clip_window_offset, 0.0, |s| match &layer {
                Some(layer) => {
                    if first_copy_visible {
                        s.draw_layer(layer);
                    }
                    if second_copy_visible {
                        s.translate_with(second_copy_offset, 0.0, |s| s.draw_layer(layer));
                    }
                }
                // (not attached: drawn as it is)
                None => {
                    if first_copy_visible {
                        s.draw_content();
                    }
                    if second_copy_visible {
                        s.translate_with(second_copy_offset, 0.0, |s| s.draw_content());
                    }
                }
            });
        });
    }
}
