//! Material's pull to refresh, as Compose's `androidx.compose.material3.pulltorefresh` package (PullToRefresh.kt):
//! the container of what scrolls with the gesture and its indicator ([`PullToRefreshBox`]), the modifier that lets a
//! pull down at the start of what scrolls inside ask for a refresh ([`PullToRefreshModifiers::pull_to_refresh`]), its
//! state ([`PullToRefreshState`], [`remember_pull_to_refresh_state`]), and the defaults ([`PullToRefreshDefaults`]:
//! the indicators — the arrow growing with the pull and the spinner, the loading indicator —, their container,
//! shapes, colors, threshold and elevations).

use crate::internal::float_producer::FloatProducer;
use crate::loading_indicator::{
    ContainedLoadingIndicator, IndeterminateContainedLoadingIndicator, LoadingIndicatorDefaults,
};
use crate::material_theme::MaterialTheme;
use crate::progress_indicator::IndeterminateCircularProgressIndicator;
use crate::tokens::{ElevationTokens, MotionSchemeKeyTokens};
use kompoz_animation::{
    Animatable, AnimationSpec, AnimationVector1D, Crossfade, VectorConverter, animate_float_as_state,
};
use kompoz_foundation::Canvas;
use kompoz_foundation::gestures::LocalFuture;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    CircleShape, Color, Density, Dp, Offset, Path, PathFillType, Rect, Shape, StrokeCap, StrokeJoin,
};
use kompoz_graphics::{DrawStyle, Stroke};
use kompoz_runtime::{
    MutableState, Saveable, Saver, derived_state_of, kompozable, kompozable_block, mutable_float_state_of, named,
    named_args, remember,
};
use kompoz_ui::draw::DrawScope;
use kompoz_ui::graphics_context::ClipOp;
use kompoz_ui::input::{FlingFuture, NestedScrollConnection, NestedScrollNode, NestedScrollSource};
use kompoz_ui::layout::{GraphicsLayerScope, LayerBlock};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::semantics::ProgressBarRangeInfo;
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Alignment, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------
// PullToRefreshBox

/// A container for scrollable content adding the pull-to-refresh gesture at its start (Compose's `PullToRefreshBox`):
/// `content` in a Box of `modifier` and `content_alignment` with [`PullToRefreshModifiers::pull_to_refresh`]
/// (`is_refreshing`, `state` — by default a remembered one —, `enabled`, `threshold`, `on_refresh`), `indicator` drawn
/// on top (None: [`PullToRefreshDefaults::Indicator`] of `state` and `is_refreshing` at the top center — Compose's
/// default lambda, which reads other parameters). Compose's HIDDEN overload without `enabled` and `threshold` isn't
/// ported (Kotlin can't call it).
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn PullToRefreshBox(
    is_refreshing: bool,
    on_refresh: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(remember_pull_to_refresh_state())] state: Rc<dyn PullToRefreshState>,
    #[default(Alignment::TopStart)] content_alignment: Alignment,
    #[default(None)] indicator: Option<Rc<dyn Fn()>>,
    #[default(true)] enabled: bool,
    #[default(PullToRefreshDefaults::PositionalThreshold)] threshold: Dp,
    content: impl Fn() + 'static,
) {
    let indicator_state = state.clone();
    Box(
        modifier = modifier.pull_to_refresh(
            is_refreshing,
            state,
            enabled = enabled,
            threshold = threshold,
            on_refresh = on_refresh,
        ),
        content_alignment = content_alignment,
        content = || {
            content();
            match &indicator {
                Some(indicator) => indicator(),
                None => PullToRefreshDefaults.Indicator(
                    indicator_state.clone(),
                    is_refreshing,
                    modifier = Modifier.align(Alignment::TopCenter),
                ),
            }
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Modifier.pullToRefresh

/// The pull-to-refresh modifier (Compose's extension function `Modifier.pullToRefresh`).
#[named(trait PullToRefreshModifiers)]
impl Modifier {
    /// Lets a pull down at the start of what scrolls inside ask for a refresh (Compose's `Modifier.pullToRefresh`;
    /// PullToRefreshBox applies it): a nested scroll connection taking what is pulled down past the start (half of
    /// it counts, slowing past `threshold`, at most twice past it) and giving it back first when pushed up again,
    /// into `state`'s distance fraction; let go past `threshold`, `on_refresh` is called. Not `enabled`, or while
    /// `state` animates, it takes nothing. `is_refreshing` says where the state goes once no gesture is going on:
    /// the threshold (1) while refreshing, else hidden (0).
    fn pull_to_refresh(
        self,
        is_refreshing: bool,
        state: Rc<dyn PullToRefreshState>,
        #[default(true)] enabled: bool,
        #[default(PullToRefreshDefaults::PositionalThreshold)] threshold: Dp,
        #[memoized] on_refresh: impl Fn() + 'static,
    ) -> Modifier {
        let on_refresh_id = on_refresh.id();
        let on_refresh: Rc<dyn Fn()> = Rc::new(on_refresh.into_value());
        self.then_element(PullToRefreshElement { is_refreshing, on_refresh, on_refresh_id, enabled, state, threshold })
    }
}

/// Compose's internal `PullToRefreshElement`: equal for the same values, the same callback (by its identity as a
/// memoized closure) and the same state.
struct PullToRefreshElement {
    is_refreshing: bool,
    on_refresh: Rc<dyn Fn()>,
    on_refresh_id: Option<u64>,
    enabled: bool,
    state: Rc<dyn PullToRefreshState>,
    threshold: Dp,
}

/// Whether two states are the same object (Kotlin's `equals` of an interface's implementations: by identity).
fn same_state(a: &Rc<dyn PullToRefreshState>, b: &Rc<dyn PullToRefreshState>) -> bool {
    std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b))
}

impl PartialEq for PullToRefreshElement {
    fn eq(&self, other: &Self) -> bool {
        self.is_refreshing == other.is_refreshing
            && self.enabled == other.enabled
            && self.on_refresh_id.is_some()
            && self.on_refresh_id == other.on_refresh_id
            && same_state(&self.state, &other.state)
            && self.threshold == other.threshold
    }
}

impl ModifierNodeElement for PullToRefreshElement {
    type Node = PullToRefreshModifierNode;

    fn create(&self) -> PullToRefreshModifierNode {
        PullToRefreshModifierNode {
            core: Rc::new_cyclic(|this| PullToRefreshCore {
                this: this.clone(),
                is_refreshing: Cell::new(self.is_refreshing),
                on_refresh: RefCell::new(self.on_refresh.clone()),
                enabled: Cell::new(self.enabled),
                state: RefCell::new(self.state.clone()),
                threshold: Cell::new(self.threshold),
                vertical_offset: mutable_float_state_of(0.0),
                distance_pulled: mutable_float_state_of(0.0),
                handle: RefCell::new(None),
            }),
            nested_scroll_node: None,
        }
    }

    fn update(&self, node: &mut PullToRefreshModifierNode) {
        let core = &node.core;
        *core.on_refresh.borrow_mut() = self.on_refresh.clone();
        core.enabled.set(self.enabled);
        *core.state.borrow_mut() = self.state.clone();
        core.threshold.set(self.threshold);
        if core.is_refreshing.get() != self.is_refreshing {
            core.is_refreshing.set(self.is_refreshing);
            core.update();
        }
    }

    fn name(&self) -> &'static str {
        "PullToRefreshModifierNode"
    }
}

/// Compose's internal `PullToRefreshModifierNode`: a delegating node, its nested scroll node's connection what it
/// shares with the coroutines it starts ([`PullToRefreshCore`]).
struct PullToRefreshModifierNode {
    core: Rc<PullToRefreshCore>,
    nested_scroll_node: Option<DelegatedNode<NestedScrollNode>>,
}

impl ModifierNode for PullToRefreshModifierNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        if self.nested_scroll_node.is_none() {
            let connection: Rc<dyn NestedScrollConnection> = self.core.clone();
            self.nested_scroll_node =
                Some(cx.delegate(kompoz_ui::input::nested_scroll_modifier_node(connection, None)));
        }
        *self.core.handle.borrow_mut() = Some(cx.handle());
        let core = &self.core;
        let (state, is_refreshing) = (core.state(), core.is_refreshing.get());
        core.launch(async move { state.snap_to(if is_refreshing { 1.0 } else { 0.0 }).await });
        core.vertical_offset.set(if is_refreshing { core.threshold_px() as f32 } else { 0.0 });
    }

    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

/// What the node shares with its nested scroll connection and the coroutines it starts: its element's values
/// (Kotlin's `var`s of the node), the pull's distances (snapshot states, as Compose's `mutableFloatStateOf`s) and
/// the node's handle (its density, its coroutine scope, whether it is attached).
struct PullToRefreshCore {
    this: Weak<PullToRefreshCore>,
    is_refreshing: Cell<bool>,
    on_refresh: RefCell<Rc<dyn Fn()>>,
    enabled: Cell<bool>,
    state: RefCell<Rc<dyn PullToRefreshState>>,
    threshold: Cell<Dp>,
    vertical_offset: MutableState<f32>,
    distance_pulled: MutableState<f32>,
    handle: RefCell<Option<NodeHandle>>,
}

impl PullToRefreshCore {
    fn state(&self) -> Rc<dyn PullToRefreshState> {
        self.state.borrow().clone()
    }

    fn handle(&self) -> Option<NodeHandle> {
        self.handle.borrow().clone()
    }

    fn adjusted_distance_pulled(&self) -> f32 {
        self.distance_pulled.get() * DragMultiplier
    }

    /// The threshold in px (Compose's `thresholdPx`, `requireDensity()`: panics once detached).
    #[track_caller]
    fn threshold_px(&self) -> i32 {
        let handle = self.handle().expect("Cannot obtain node coordinator. Is the Modifier.Node attached?");
        handle.require_density().round_to_px(self.threshold.get())
    }

    fn progress(&self) -> f32 {
        self.adjusted_distance_pulled() / self.threshold_px() as f32
    }

    fn is_attached(&self) -> bool {
        self.handle().is_some_and(|h| h.is_attached())
    }

    /// Kotlin's `coroutineScope.launch { … }` of the node (nothing once it is detached: its scope is cancelled).
    fn launch(&self, block: impl Future<Output = ()> + 'static) {
        if let Some(scope) = self.handle().and_then(|h| h.coroutine_scope()) {
            named_args!(scope.launch(block));
        }
    }

    /// Compose's `update`: to the threshold while refreshing, else hidden.
    fn update(&self) {
        let Some(this) = self.this.upgrade() else { return };
        self.launch(async move {
            if !this.is_refreshing.get() {
                this.animate_to_hidden().await;
            } else {
                this.animate_to_threshold().await;
            }
        });
    }

    /// Helper method for nested scroll connection (Compose's `consumeAvailableOffset`).
    fn consume_available_offset(&self, available: Offset) -> Offset {
        let y = if self.is_refreshing.get() {
            0.0
        } else {
            let distance_pulled = self.distance_pulled.get();
            let new_offset = (distance_pulled + available.y).max(0.0);
            let drag_consumed = new_offset - distance_pulled;
            self.distance_pulled.set(new_offset);
            self.vertical_offset.set(self.calculate_vertical_offset());
            drag_consumed
        };
        Offset::new(0.0, y)
    }

    /// Helper method for nested scroll connection; calls the refresh callback when triggered (Compose's
    /// `onRelease`).
    async fn on_release(&self, velocity: f32) -> f32 {
        if self.is_refreshing.get() {
            return 0.0; // Already refreshing, do nothing
        }
        // Trigger refresh
        if self.adjusted_distance_pulled() > self.threshold_px() as f32 {
            let on_refresh = self.on_refresh.borrow().clone();
            on_refresh();
        }

        let consumed = if self.distance_pulled.get() == 0.0 {
            // We are flinging without having dragged the pull refresh (for example a fling inside a list) - don't
            // consume
            0.0
        } else if velocity < 0.0 {
            // If the velocity is negative, the fling is upwards, and we don't want to prevent the list from
            // scrolling
            0.0
        } else {
            // We are showing the indicator, and the fling is downwards - consume everything
            velocity
        };

        self.animate_to_hidden().await;

        self.distance_pulled.set(0.0);

        consumed
    }

    /// Compose's `calculateVerticalOffset`.
    fn calculate_vertical_offset(&self) -> f32 {
        let threshold_px = self.threshold_px() as f32;
        let adjusted_distance_pulled = self.adjusted_distance_pulled();
        // If drag hasn't gone past the threshold, the position is the adjustedDistancePulled.
        if adjusted_distance_pulled <= threshold_px {
            adjusted_distance_pulled
        } else {
            // How far beyond the threshold pull has gone, as a percentage of the threshold.
            let overshoot_percent = self.progress().abs() - 1.0;
            // Limit the overshoot to 200%. Linear between 0 and 200.
            let linear_tension = overshoot_percent.clamp(0.0, 2.0);
            // Non-linear tension. Increases with linearTension, but at a decreasing rate.
            let tension_percent = linear_tension - linear_tension.powi(2) / 4.0;
            // The additional offset beyond the threshold.
            let extra_offset = threshold_px * tension_percent;
            threshold_px + extra_offset
        }
    }

    /// Compose's private `animateToThreshold`: its `finally` (run however the animation ends, cancelled too) puts
    /// the distances at the threshold while the node is attached.
    async fn animate_to_threshold(&self) {
        struct Finally<'a>(&'a PullToRefreshCore);
        impl Drop for Finally<'_> {
            fn drop(&mut self) {
                let core = self.0;
                if core.is_attached() {
                    // Don't read density if the node is not attached. This will get updated next time the node is
                    // attached
                    let threshold_px = core.threshold_px() as f32;
                    core.distance_pulled.set(threshold_px);
                    core.vertical_offset.set(threshold_px);
                }
            }
        }
        let _finally = Finally(self);
        let animation = self.state().animate_to_threshold();
        animation.await;
    }

    /// Compose's private `animateToHidden`: its `finally` puts the distances back at 0.
    async fn animate_to_hidden(&self) {
        struct Finally<'a>(&'a PullToRefreshCore);
        impl Drop for Finally<'_> {
            fn drop(&mut self) {
                self.0.distance_pulled.set(0.0);
                self.0.vertical_offset.set(0.0);
            }
        }
        let _finally = Finally(self);
        let animation = self.state().animate_to_hidden();
        animation.await;
    }
}

impl NestedScrollConnection for PullToRefreshCore {
    fn on_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        if self.state().is_animating() || !self.enabled.get() {
            Offset::Zero
        } else if source == NestedScrollSource::UserInput && available.y < 0.0 {
            // Swiping up
            self.consume_available_offset(available)
        } else {
            Offset::Zero
        }
    }

    fn on_post_scroll(&self, _consumed: Offset, available: Offset, source: NestedScrollSource) -> Offset {
        if self.state().is_animating() || !self.enabled.get() {
            Offset::Zero
        } else if source == NestedScrollSource::UserInput {
            // Swiping down
            let new_offset = self.consume_available_offset(available);
            if let Some(this) = self.this.upgrade() {
                self.launch(async move {
                    let state = this.state();
                    if !state.is_animating() {
                        let fraction = this.vertical_offset.get() / this.threshold_px() as f32;
                        state.snap_to(fraction).await;
                    }
                });
            }
            new_offset
        } else {
            Offset::Zero
        }
    }

    fn on_pre_fling(&self, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move { Velocity::new(0.0, self.on_release(available.y).await) })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// PullToRefreshDefaults

/// The defaults of the pull to refresh (Compose's `object PullToRefreshDefaults`; a unit value:
/// `PullToRefreshDefaults.IndicatorBox(…)`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PullToRefreshDefaults;

impl PullToRefreshDefaults {
    /// The default shape of the indicator (Compose's deprecated `shape`): a circle.
    #[deprecated(note = "Use indicatorShape instead")]
    pub fn shape() -> Arc<dyn Shape> {
        Arc::new(CircleShape)
    }

    /// The default shape of the indicator (Compose's `indicatorShape`): a circle.
    pub fn indicator_shape() -> Arc<dyn Shape> {
        Arc::new(CircleShape)
    }

    /// The default container color of the indicator (Compose's deprecated composable `containerColor`): the
    /// theme's surface container high.
    #[deprecated(note = "Use indicatorContainerColor instead")]
    pub fn container_color() -> Color {
        MaterialTheme::color_scheme().surface_container_high
    }

    /// The default container color of the indicator (Compose's composable `indicatorContainerColor`): the theme's
    /// surface container high.
    pub fn indicator_container_color() -> Color {
        MaterialTheme::color_scheme().surface_container_high
    }

    /// The default container color of the loading indicator (Compose's composable
    /// `loadingIndicatorContainerColor`): the contained loading indicator's.
    pub fn loading_indicator_container_color() -> Color {
        LoadingIndicatorDefaults::contained_container_color()
    }

    /// The default color of the indicator (Compose's composable `indicatorColor`): the theme's on surface variant.
    pub fn indicator_color() -> Color {
        MaterialTheme::color_scheme().on_surface_variant
    }

    /// The default active indicator color of the loading indicator (Compose's composable `loadingIndicatorColor`):
    /// the contained loading indicator's.
    pub fn loading_indicator_color() -> Color {
        LoadingIndicatorDefaults::contained_indicator_color()
    }

    /// The default refresh threshold (Compose's `PositionalThreshold`): 80 dp.
    pub const PositionalThreshold: Dp = Dp(80.0);

    /// The default maximum distance the indicators can be pulled before a refresh is triggered (Compose's
    /// `IndicatorMaxDistance`): the threshold.
    pub const IndicatorMaxDistance: Dp = PullToRefreshDefaults::PositionalThreshold;

    /// The default elevation of an [`IndicatorBox`](Self::IndicatorBox) around the indicator (Compose's
    /// `Elevation`).
    pub const Elevation: Dp = ElevationTokens::Level2;

    /// The default elevation of an [`IndicatorBox`](Self::IndicatorBox) around the loading indicator (Compose's
    /// `LoadingIndicatorElevation`).
    pub const LoadingIndicatorElevation: Dp = ElevationTokens::Level0;
}

#[named]
impl PullToRefreshDefaults {
    /// What gives a pull-to-refresh indicator its size, place, clip, shadow and background, for indicators of
    /// one's own (Compose's `IndicatorBox`; the indicator's container): a 40 dp Box of `shape` filled with
    /// `container_color` (unspecified: none), `content` centered in it, coming down from under the top edge it rests
    /// at as `state`'s distance fraction grows (all the way out at `max_distance`), drawn clipped at that edge, with
    /// a shadow of `elevation` while pulled out or `is_refreshing`.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn IndicatorBox(
        &self,
        state: Rc<dyn PullToRefreshState>,
        is_refreshing: bool,
        #[default(Modifier)] modifier: Modifier,
        #[default(PullToRefreshDefaults::IndicatorMaxDistance)] max_distance: Dp,
        #[default(PullToRefreshDefaults::indicator_shape())] shape: Arc<dyn Shape>,
        #[default(Color::Unspecified)] container_color: Color,
        #[default(PullToRefreshDefaults::Elevation)] elevation: Dp,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            let layer_shape = shape.clone();
            Box(
                modifier = modifier
                    .size(SpinnerContainerSize)
                    .draw_with_content(|c| {
                        c.clip_rect_with(-f32::MAX, 0.0, f32::MAX, f32::MAX, ClipOp::Intersect, |c| c.draw_content());
                    })
                    .layout(move |scope, measurable, constraints| {
                        let placeable = measurable.measure(constraints);
                        let (state, shape) = (state.clone(), layer_shape.clone());
                        let layer_block: LayerBlock = Rc::new(move |layer: &mut GraphicsLayerScope| {
                            let distance_fraction = state.distance_fraction();
                            let show_elevation = distance_fraction > 0.0 || is_refreshing;
                            let density = layer.density();
                            layer.translation_y =
                                distance_fraction * density.round_to_px(max_distance) as f32 - layer.size().height;
                            layer.shadow_elevation = if show_elevation { density.to_px(elevation) } else { 0.0 };
                            layer.shape = shape.clone();
                            layer.clip = true;
                        });
                        let (width, height) = (placeable.width(), placeable.height());
                        named_args!(scope.layout(width, height, move |p| {
                            p.place_with_layer_block(&placeable, Default::default(), layer_block.clone(), 0.0)
                        }))
                    })
                    .background(container_color, shape),
                content_alignment = Alignment::Center,
                content = || content(),
            );
        })
    }

    /// The default indicator of [`PullToRefreshBox`] (Compose's `Indicator`): an [`IndicatorBox`](Self::IndicatorBox)
    /// of `container_color` coming out with `state` up to `max_distance`, crossfading (the motion scheme's default
    /// effects) between the arrow growing with the pull and, `is_refreshing`, an endless circular progress indicator,
    /// both in `color`.
    #[track_caller]
    pub fn Indicator(
        &self,
        state: Rc<dyn PullToRefreshState>,
        is_refreshing: bool,
        #[default(Modifier)] modifier: Modifier,
        #[default(PullToRefreshDefaults::indicator_container_color())] container_color: Color,
        #[default(PullToRefreshDefaults::indicator_color())] color: Color,
        #[default(PullToRefreshDefaults::IndicatorMaxDistance)] max_distance: Dp,
    ) {
        kompozable_block!({
            let progress_state = state.clone();
            self.IndicatorBox(
                state,
                is_refreshing,
                modifier = modifier,
                container_color = container_color,
                max_distance = max_distance,
                content = move || {
                    let progress_state = progress_state.clone();
                    // TODO Load the motionScheme tokens from the component tokens file
                    Crossfade(
                        is_refreshing,
                        animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>(),
                        content = move |refreshing| {
                            if refreshing {
                                IndeterminateCircularProgressIndicator(
                                    stroke_width = StrokeWidth,
                                    color = color,
                                    modifier = Modifier.size(SpinnerSize),
                                );
                            } else {
                                let s = progress_state.clone();
                                CircularArrowProgressIndicator(
                                    FloatProducer::new(move || s.distance_fraction()),
                                    color,
                                );
                            }
                        },
                    );
                },
            );
        })
    }

    /// A loading indicator for [`PullToRefreshBox`] (Compose's `LoadingIndicator`): an
    /// [`IndicatorBox`](Self::IndicatorBox) of the loading indicator's size, `container_color` and `elevation`,
    /// crossfading between the contained loading indicator morphing with the pull (the whole of it turning past the
    /// threshold) and, `is_refreshing`, the endless one, both in `color`.
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn LoadingIndicator(
        &self,
        state: Rc<dyn PullToRefreshState>,
        is_refreshing: bool,
        #[default(Modifier)] modifier: Modifier,
        #[default(PullToRefreshDefaults::loading_indicator_container_color())] container_color: Color,
        #[default(PullToRefreshDefaults::loading_indicator_color())] color: Color,
        #[default(PullToRefreshDefaults::LoadingIndicatorElevation)] elevation: Dp,
        #[default(PullToRefreshDefaults::IndicatorMaxDistance)] max_distance: Dp,
    ) {
        kompozable_block!({
            let progress_state = state.clone();
            self.IndicatorBox(
                state,
                is_refreshing,
                modifier = modifier.size_wh(LoaderIndicatorWidth, LoaderIndicatorHeight),
                container_color = container_color,
                elevation = elevation,
                max_distance = max_distance,
                content = move || {
                    let progress_state = progress_state.clone();
                    // TODO Load the motionScheme tokens from the component tokens file
                    Crossfade(
                        is_refreshing,
                        animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>(),
                        content = move |refreshing| {
                            if refreshing {
                                IndeterminateContainedLoadingIndicator(
                                    // TODO Set the LoadingIndicator colors
                                    modifier = Modifier.required_size_wh(LoaderIndicatorWidth, LoaderIndicatorHeight),
                                    container_color = container_color,
                                    indicator_color = color,
                                );
                            } else {
                                // The LoadingIndicator will rotate and morph for a coerced progress value of 0 to 1.
                                // When the state's distanceFraction is above one, we rotate the entire component we
                                // have a continuous rotation until the refreshing flag is true.
                                let (s, d) = (progress_state.clone(), progress_state.clone());
                                ContainedLoadingIndicator(
                                    // TODO Set the LoadingIndicator colors
                                    progress = move || s.distance_fraction(),
                                    modifier = Modifier
                                        .required_size_wh(LoaderIndicatorWidth, LoaderIndicatorHeight)
                                        .draw_with_content(move |c| {
                                            let progress = d.distance_fraction();
                                            if progress > 1.0 {
                                                // Start the rotation on progress - 1 (i.e. 0) to avoid a jump that
                                                // would be more noticeable on some LoadingIndicator shapes.
                                                let center = c.center();
                                                c.rotate_with(-(progress - 1.0) * 180.0, center, |c| c.draw_content());
                                            } else {
                                                c.draw_content();
                                            }
                                        }),
                                    container_color = container_color,
                                    indicator_color = color,
                                );
                            }
                        },
                    );
                },
            );
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// PullToRefreshState

/// The state of a pull to refresh, the distance its container and indicator have been pulled (Compose's interface
/// `PullToRefreshState`): each pull to refresh its own. Other progress indicators can show it too.
pub trait PullToRefreshState {
    /// How far towards the refresh threshold it is (Compose's `distanceFraction`): 0 none, 1 at the threshold's
    /// offset, more past it.
    fn distance_fraction(&self) -> f32;

    /// Whether it animates the indicator to the threshold's offset or back to hidden (Compose's `isAnimating`).
    fn is_animating(&self) -> bool;

    /// Animates the distance to the threshold, where the indicator shows while refreshing (Compose's suspending
    /// `animateToThreshold`).
    fn animate_to_threshold(&self) -> LocalFuture<'static, ()>;

    /// Animates the distance to where the indicator is hidden when idle (Compose's suspending
    /// `animateToHidden`).
    fn animate_to_hidden(&self) -> LocalFuture<'static, ()>;

    /// Snaps the indicator to `target_value`, a fraction of the threshold (Compose's suspending `snapTo`).
    fn snap_to(&self, target_value: f32) -> LocalFuture<'static, ()>;
}

/// The default pull-to-refresh state, saved and restored (Compose's `rememberPullToRefreshState`).
#[kompozable]
pub fn remember_pull_to_refresh_state() -> Rc<dyn PullToRefreshState> {
    let saver = PullToRefreshStateImpl::Saver();
    kompoz_runtime::remember_saveable_with(&saver, || Rc::new(PullToRefreshStateImpl::new()))
        as Rc<dyn PullToRefreshState>
}

/// A pull-to-refresh state (Compose's function `PullToRefreshState()`): in composition, rather
/// [`remember_pull_to_refresh_state`].
///
/// Kotlin: `PullToRefreshState(): PullToRefreshState`
pub fn PullToRefreshState() -> Rc<dyn PullToRefreshState> {
    Rc::new(PullToRefreshStateImpl::new())
}

/// The default state (Compose's internal `PullToRefreshStateImpl`): an Animatable of the distance fraction.
pub(crate) struct PullToRefreshStateImpl {
    anim: Animatable<f32, AnimationVector1D>,
}

impl PullToRefreshStateImpl {
    pub(crate) fn new() -> PullToRefreshStateImpl {
        PullToRefreshStateImpl { anim: Animatable((0.0, f32::VectorConverter)) }
    }

    /// Saves its distance fraction, restored as a state at it (Compose's `PullToRefreshStateImpl.Saver`).
    #[allow(non_snake_case)]
    pub(crate) fn Saver() -> Saver<Rc<PullToRefreshStateImpl>> {
        Saver::new(
            |state: &Rc<PullToRefreshStateImpl>| state.anim.value().save(),
            |saved| {
                let value = f32::restore(saved)?;
                Some(Rc::new(PullToRefreshStateImpl { anim: Animatable((value, f32::VectorConverter)) }))
            },
        )
    }
}

impl PullToRefreshState for PullToRefreshStateImpl {
    fn distance_fraction(&self) -> f32 {
        self.anim.value()
    }

    fn is_animating(&self) -> bool {
        self.anim.is_running()
    }

    fn animate_to_threshold(&self) -> LocalFuture<'static, ()> {
        let animation = named_args!(self.anim.animate_to(1.0));
        Box::pin(async move {
            animation.await;
        })
    }

    fn animate_to_hidden(&self) -> LocalFuture<'static, ()> {
        let animation = named_args!(self.anim.animate_to(0.0));
        Box::pin(async move {
            animation.await;
        })
    }

    fn snap_to(&self, target_value: f32) -> LocalFuture<'static, ()> {
        Box::pin(self.anim.snap_to(target_value))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The indicator's arrow

/// The default pull indicator (Compose's private `CircularArrowProgressIndicator`): an arc growing with
/// `progress` past 40 % of the threshold, its arrowhead at its end, faint until the threshold, turning a little
/// past it; a progress to accessibility while pulled.
#[kompozable]
pub(crate) fn CircularArrowProgressIndicator(progress: FloatProducer, color: Color) {
    let path = remember(|| {
        let mut path = Path::new();
        path.fill_type = PathFillType::EvenOdd;
        Rc::new(RefCell::new(path))
    });
    // TODO: Consider refactoring this sub-component utilizing Modifier.Node
    let target_alpha = {
        let progress = progress.clone();
        remember(move || derived_state_of(move || if progress.invoke() >= 1.0 { MaxAlpha } else { MinAlpha }))
    };
    // TODO Load the motionScheme tokens from the component tokens file
    let alpha_state = animate_float_as_state(
        target_alpha.get(),
        animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>() as Arc<dyn AnimationSpec<f32>>,
    );
    let semantics_progress = progress.clone();
    Canvas(
        modifier = Modifier
            .clear_and_set_semantics(move |c| {
                let progress = semantics_progress.invoke();
                if progress > 0.0 {
                    c.set_progress_bar_range_info(ProgressBarRangeInfo::new(progress, 0.0..=1.0, 0));
                }
            })
            .size(SpinnerSize),
        on_draw = move |d: &mut DrawScope| {
            let values = arrow_values(progress.invoke());
            let alpha = alpha_state.get();
            let center = d.center();
            d.rotate_with(values.rotation, center, |d| {
                let arc_radius = d.to_px(ArcRadius) + d.to_px(StrokeWidth) / 2.0;
                let arc_bounds = Rect::from_center(d.size().center(), arc_radius);
                draw_circular_indicator(d, color, alpha, &values, arc_bounds, StrokeWidth);
                draw_arrow(d, &mut path.borrow_mut(), arc_bounds, color, alpha, &values, StrokeWidth);
            });
        },
    );
}

/// Compose's private `DrawScope.drawCircularIndicator`.
fn draw_circular_indicator(
    d: &mut DrawScope,
    color: Color,
    alpha: f32,
    values: &ArrowValues,
    arc_bounds: Rect,
    stroke_width: Dp,
) {
    d.draw_arc_with(
        color,
        values.start_angle,
        values.end_angle - values.start_angle,
        false,
        arc_bounds.top_left(),
        arc_bounds.size(),
        alpha,
        DrawStyle::Stroke(stroke(d.density(), stroke_width, StrokeCap::Butt)),
        None,
        Default::default(),
    );
}

/// Kotlin's `Stroke(width = width.toPx(), cap = cap)`.
fn stroke(density: Density, width: Dp, cap: StrokeCap) -> Stroke {
    Stroke::new(density.to_px(width), Stroke::DefaultMiter, cap, StrokeJoin::Miter, None)
}

/// Compose's private `ArrowValues`.
struct ArrowValues {
    rotation: f32,
    start_angle: f32,
    end_angle: f32,
    scale: f32,
}

/// Compose's private `ArrowValues(progress)`.
fn arrow_values(progress: f32) -> ArrowValues {
    // Discard first 40% of progress. Scale remaining progress to full range between 0 and 100%.
    let adjusted_percent = (progress.min(1.0) - 0.4).max(0.0) * 5.0 / 3.0;
    // How far beyond the threshold pull has gone, as a percentage of the threshold.
    let overshoot_percent = progress.abs() - 1.0;
    // Limit the overshoot to 200%. Linear between 0 and 200.
    let linear_tension = overshoot_percent.clamp(0.0, 2.0);
    // Non-linear tension. Increases with linearTension, but at a decreasing rate.
    let tension_percent = linear_tension - linear_tension.powi(2) / 4.0;

    // Calculations based on SwipeRefreshLayout specification.
    let end_trim = adjusted_percent * MaxProgressArc;
    let rotation = (-0.25 + 0.4 * adjusted_percent + tension_percent) * 0.5;
    let start_angle = rotation * 360.0;
    let end_angle = (rotation + end_trim) * 360.0;
    let scale = adjusted_percent.min(1.0);

    ArrowValues { rotation, start_angle, end_angle, scale }
}

/// Compose's private `DrawScope.drawArrow`.
fn draw_arrow(
    d: &mut DrawScope,
    arrow: &mut Path,
    bounds: Rect,
    color: Color,
    alpha: f32,
    values: &ArrowValues,
    stroke_width: Dp,
) {
    let (arrow_width, arrow_height) = (d.to_px(ArrowWidth), d.to_px(ArrowHeight));
    arrow.reset();
    arrow.move_to(0.0, 0.0); // Move to left corner
    // Line to tip of arrow
    arrow.line_to(arrow_width * values.scale / 2.0, arrow_height * values.scale);
    arrow.line_to(arrow_width * values.scale, 0.0); // Line to right corner

    let radius = bounds.width().min(bounds.height()) / 2.0;
    let inset = arrow_width * values.scale / 2.0;
    let stroke_width_px = d.to_px(stroke_width);
    arrow.translate(Offset::new(radius + bounds.center().x - inset, bounds.center().y - stroke_width_px));
    let center = d.center();
    let style = DrawStyle::Stroke(stroke(d.density(), stroke_width, Stroke::DefaultCap));
    let arrow = &*arrow;
    d.rotate_with(values.end_angle - stroke_width_px, center, |d| {
        d.draw_path_with(arrow, color, alpha, style, None, Default::default());
    });
}

const MaxProgressArc: f32 = 0.8;

/// The default stroke width of the indicator (Compose's private `StrokeWidth`).
const StrokeWidth: Dp = Dp(2.5);
const ArcRadius: Dp = Dp(5.5);
/// (ArcRadius + StrokeWidth) × 2 (Compose's internal `SpinnerSize`).
pub(crate) const SpinnerSize: Dp = Dp(16.0);
pub(crate) const SpinnerContainerSize: Dp = Dp(40.0);
/// The loading indicator's container height (Compose's internal `LoaderIndicatorHeight`).
pub(crate) const LoaderIndicatorHeight: Dp = LoadingIndicatorDefaults::ContainerHeight;
/// The loading indicator's container width (Compose's internal `LoaderIndicatorWidth`).
pub(crate) const LoaderIndicatorWidth: Dp = LoadingIndicatorDefaults::ContainerWidth;
const ArrowWidth: Dp = Dp(10.0);
const ArrowHeight: Dp = Dp(5.0);

// Values taken from SwipeRefreshLayout
const MinAlpha: f32 = 0.3;
const MaxAlpha: f32 = 1.0;

/// What the distance pulled is multiplied by to give the adjusted distance pulled, the indicator's position below
/// the threshold (past it, the position comes from the progress) (Compose's private `DragMultiplier`).
const DragMultiplier: f32 = 0.5;

use std::future::Future;

#[cfg(test)]
mod tests {
    //! The default state's saver (private, so tested here; the indicators are tested through
    //! `PullToRefreshDefaults.Indicator` in tests/swipe_pull.rs).

    use super::*;

    #[test]
    fn the_default_state_saves_its_fraction() {
        kompoz_runtime::set_ui_thread();
        let saver = PullToRefreshStateImpl::Saver();
        let state = Rc::new(PullToRefreshStateImpl { anim: Animatable((0.6, f32::VectorConverter)) });
        let restored = saver.restore(&saver.save(&state)).expect("restored");
        assert!(!Rc::ptr_eq(&state, &restored) && restored.distance_fraction() == 0.6 && !restored.is_animating());
    }
}
