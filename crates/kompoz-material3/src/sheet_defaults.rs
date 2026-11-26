//! Material's bottom sheets' common parts, as Compose's SheetDefaults.kt: the sheet itself in the composition
//! ([`BottomSheet`]: no dialog window, no scrim), its state ([`SheetState`], [`SheetValue`]), the defaults
//! ([`BottomSheetDefaults`]: shapes, colors, elevation, insets, the peek height and maximum width, the drag
//! handle), the remembered state ([`remember_bottom_sheet_state`], with the state's set of enabled values: material3
//! 1.5.0-alpha20's, the rest of this crate being 1.5.0-alpha17's), and what [`ModalBottomSheet`](crate::ModalBottomSheet) and
//! [`BottomSheetScaffold`](crate::BottomSheetScaffold) share: the drag handle in its tooltip, the nested scroll
//! connection moving the sheet before (and after) what scrolls in it, the predictive back scaling, and the
//! vertical scaling covering a bouncy opening's overshoot.

// (the deprecated SheetStateSkipping and Saver overload are defined here)
#![allow(deprecated)]

use crate::color_scheme::content_color_for;
use crate::internal::back_handler::{PredictiveBack, PredictiveBackHandler};
use crate::internal::draggable_anchors::DraggableAnchorsModifiers;
use crate::internal::strings::{Strings, get_string};
use crate::material_theme::MaterialTheme;
use crate::surface::Surface;
use crate::text::Text;
use crate::tokens::{ScrimTokens, SheetBottomTokens};
use crate::tooltip::{
    TooltipAnchorPosition, TooltipBox, TooltipDefaults, TooltipScope, TooltipScopeComposables, remember_tooltip_state,
};
use kompoz_animation::{AnimationSpec, FastOutSlowInEasing, FiniteAnimationSpec, TweenSpec, snap, tween};
use kompoz_coroutines::{CoroutineScope, Flow, remember_coroutine_scope};
use kompoz_foundation::gestures::{
    AnchoredDragScope, AnchoredDraggableDefaults, AnchoredDraggableState, DraggableAnchors, DraggableAnchorsConfig,
    FlingBehavior, LocalFuture, Orientation, ScrollScope, TargetedFlingBehavior, anchored_draggable_state_confirming,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Density, Dp, Offset, Shape, lerp};
use kompoz_runtime::kt::{KtSet, panic_kt, set_of};
use kompoz_runtime::{
    DerivedState, Memoized, Saveable, SavedValue, Saver, SideEffect, derived_state_of, kompozable, kompozable_block,
    kompozable_lambda, named, named_args, remember, remember_keyed, remember_saveable_keyed_with,
};
use kompoz_ui::input::{FlingFuture, NestedScrollConnection, NestedScrollSource};
use kompoz_ui::layout::{GraphicsLayerScope, TransformOrigin};
use kompoz_ui::platform::{LocalDensity, LocalViewConfiguration};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Alignment, Modifier};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use SheetValue::{Expanded, Hidden, PartiallyExpanded};

// ---------------------------------------------------------------------------------------------------------------
// BottomSheet

/// A bottom sheet in the composition itself (Compose's `BottomSheet`): unlike [`ModalBottomSheet`]
/// (crate::ModalBottomSheet), no dialog window of its own, no scrim, the rest of the screen not blocked — drawn
/// where it is placed (in a Box, say), sharing its parent's input. `state` (None: a remembered
/// [`remember_bottom_sheet_state`] hidden to start with, material3 1.5.0-alpha20's default) says where it is; it opens,
/// moves and hides by the theme's motion; dragged, flung, or its drag handle clicked, it settles on its anchors
/// (hidden below, half way when taller than half its room, expanded at its height) — `on_dismiss_request` called
/// when it ends hidden. Back (`back_handler_enabled`) scales it down with a predictive back gesture, then takes a
/// sheet expanded with a half way stop there, else hides it. It is at most `max_width` wide (unspecified: the
/// whole width); `drag_handle` (None: none) on top, then `content` in a Column padded by `content_window_insets`;
/// on a [`Surface`] of `shape`, `container_color`, `content_color` (None: the container's content color),
/// `tonal_elevation` and `shadow_elevation`. `gestures_enabled` false: it takes no drags.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn BottomSheet(
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<SheetState>,
    #[default(|| {})] on_dismiss_request: impl Fn() + 'static,
    #[default(BottomSheetDefaults::SheetMaxWidth)] max_width: Dp,
    #[default(true)] gestures_enabled: bool,
    #[default(true)] back_handler_enabled: bool,
    #[default(default_drag_handle())] drag_handle: Option<Rc<dyn Fn()>>,
    #[default(|| BottomSheetDefaults::standard_window_insets())] content_window_insets: impl Fn() -> WindowInsets + 'static,
    #[default(BottomSheetDefaults::ExpandedShape())] shape: Arc<dyn Shape>,
    #[default(BottomSheetDefaults::ContainerColor())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(BottomSheetDefaults::Elevation)] tonal_elevation: Dp,
    #[default(Dp(0.0))] shadow_elevation: Dp,
    content: impl Fn() + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_bottom_sheet_state(initial_value = Hidden),
    };
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    let motion_scheme = MaterialTheme::motion_scheme();
    let show_motion = motion_scheme.default_spatial_spec::<f32>();
    let hide_motion = motion_scheme.fast_effects_spec::<f32>();
    let anchored_draggable_motion = motion_scheme.default_spatial_spec::<f32>();
    {
        let state = state.clone();
        SideEffect(move || {
            state.set_show_motion_spec(show_motion);
            state.set_hide_motion_spec(hide_motion);
            state.set_anchored_draggable_motion_spec(anchored_draggable_motion);
        });
    }

    let predictive_back_progress = remember(|| kompoz_animation::Animatable(0.0f32));
    let scope = remember_coroutine_scope();
    let on_dismiss_request: Rc<dyn Fn()> = Rc::new(on_dismiss_request);
    let settle_to_dismiss: Rc<dyn Fn()> = {
        let (state, scope, progress, on_dismiss_request) =
            (state.clone(), scope.clone(), predictive_back_progress.clone(), on_dismiss_request.clone());
        Rc::new(move || {
            if state.current_value() == Expanded && state.has_partially_expanded_state() {
                let s = state.clone();
                named_args!(scope.launch(async move { s.partial_expand().await }));
                let p = progress.clone();
                named_args!(scope.launch(async move {
                    named_args!(p.animate_to(0.0)).await;
                }));
            } else {
                let (s, on_dismiss_request) = (state.clone(), on_dismiss_request.clone());
                launch_invoking_on_completion(&scope, state.hide(), move || {
                    if !s.is_visible() {
                        on_dismiss_request();
                    }
                });
            }
        })
    };

    {
        let (progress, scope) = (predictive_back_progress.clone(), scope.clone());
        PredictiveBackHandler(
            enabled = back_handler_enabled && state.is_visible(),
            on_back = move |back_progress: Flow<kompoz_ui::back::BackEventCompat>| {
                let (progress, scope, settle_to_dismiss) = (progress.clone(), scope.clone(), settle_to_dismiss.clone());
                async move {
                    // (Kotlin's `catch (e: CancellationException) { predictiveBackProgress.animateTo(0f) }`: a
                    // called-off gesture's coroutine is dropped here, running nothing more, so the animation back
                    // is launched in the sheet's scope as it goes)
                    let mut animate_back_if_cancelled = AnimateBackIfCancelled(Some((progress.clone(), scope)));
                    let steps = progress.clone();
                    back_progress
                        .collect_async(move |back_event| {
                            let steps = steps.clone();
                            async move {
                                steps.snap_to(PredictiveBack::transform(back_event.progress)).await;
                            }
                        })
                        .await;
                    animate_back_if_cancelled.0 = None;
                    settle_to_dismiss();
                }
            },
        );
    }
    BottomSheetImpl(
        predictive_back_progress.value(),
        modifier = modifier,
        state = Some(state),
        on_dismiss_request = move || on_dismiss_request(),
        max_width = max_width,
        gestures_enabled = gestures_enabled,
        shape = shape,
        container_color = container_color,
        content_color = Some(content_color),
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        drag_handle = drag_handle,
        content_window_insets = content_window_insets,
        content = content,
    );
}

/// Kotlin's `scope.launch { block }.invokeOnCompletion { on_completion() }`: `on_completion` runs when the
/// coroutine is done or cancelled.
pub(crate) fn launch_invoking_on_completion(
    scope: &CoroutineScope,
    block: impl Future<Output = ()> + 'static,
    on_completion: impl FnOnce() + 'static,
) {
    let on_completion = OnCompletion(Some(Box::new(on_completion)));
    named_args!(scope.launch(async move {
        let _on_completion = on_completion;
        block.await;
    }));
}

/// Runs its handler when dropped: the coroutine holding it done or cancelled.
struct OnCompletion(Option<Box<dyn FnOnce()>>);

impl Drop for OnCompletion {
    fn drop(&mut self) {
        if let Some(handler) = self.0.take() {
            handler();
        }
    }
}

/// Animates a predictive back's progress back to 0 when dropped armed: the gesture called off while it collects.
struct AnimateBackIfCancelled(
    Option<(kompoz_animation::Animatable<f32, kompoz_animation::AnimationVector1D>, CoroutineScope)>,
);

impl Drop for AnimateBackIfCancelled {
    fn drop(&mut self) {
        if let Some((progress, scope)) = self.0.take() {
            named_args!(scope.launch(async move {
                named_args!(progress.animate_to(0.0)).await;
            }));
        }
    }
}

/// The default drag handle slot: [`BottomSheetDefaults::DragHandle`] (Kotlin's default lambda, one for good so a
/// sheet given it can be skipped).
pub(crate) fn default_drag_handle() -> Option<Rc<dyn Fn()>> {
    thread_local! {
        static DEFAULT: Rc<dyn Fn()> = Rc::new(kompozable_lambda!(|| BottomSheetDefaults.DragHandle()));
    }
    Some(DEFAULT.with(Rc::clone))
}

/// The sheet's content, its predictive back progress given (Compose's internal `BottomSheetImpl`, "refactored to
/// enable predictive back testing").
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn BottomSheetImpl(
    predictive_back_progress: f32,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<SheetState>,
    #[default(|| {})] on_dismiss_request: impl Fn() + 'static,
    #[default(BottomSheetDefaults::SheetMaxWidth)] max_width: Dp,
    #[default(true)] gestures_enabled: bool,
    #[default(BottomSheetDefaults::ExpandedShape())] shape: Arc<dyn Shape>,
    #[default(BottomSheetDefaults::ContainerColor())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(BottomSheetDefaults::Elevation)] tonal_elevation: Dp,
    #[default(Dp(0.0))] shadow_elevation: Dp,
    #[default(default_drag_handle())] drag_handle: Option<Rc<dyn Fn()>>,
    #[default(|| BottomSheetDefaults::standard_window_insets())] content_window_insets: impl Fn() -> WindowInsets + 'static,
    content: impl Fn() + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_bottom_sheet_state(initial_value = Hidden),
    };
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    let bottom_sheet_pane_title = get_string(Strings::BottomSheetPaneTitle);
    let spatial_fling_spec = remembered_default_spatial_spec();
    let view_configuration = LocalViewConfiguration.current();
    let density = LocalDensity.current();
    let on_dismiss_request: Rc<dyn Fn()> = Rc::new(on_dismiss_request);

    let anchored_draggable_fling_behavior = AnchoredDraggableDefaults::fling_behavior(
        state.anchored_draggable_state(),
        Some(remembered_positional_threshold(&state)),
        Some(spatial_fling_spec),
    );

    let modal_bottom_sheet_fling_behavior = {
        let keys = (
            FlingKey(anchored_draggable_fling_behavior.clone()),
            state.clone(),
            view_configuration.maximum_fling_velocity.to_bits(),
            density.density.to_bits(),
            density.font_scale.to_bits(),
        );
        let (state, on_dismiss_request) = (state.clone(), on_dismiss_request.clone());
        remember_keyed(keys, move || {
            Rc::new(ModalBottomSheetFlingBehavior {
                anchored_draggable_fling_behavior,
                state,
                maximum_fling_velocity: view_configuration.maximum_fling_velocity,
                density,
                on_dismiss_request,
            }) as Rc<dyn FlingBehavior>
        })
    };

    let scope = remember_coroutine_scope();
    let animate_to_dismiss: Rc<dyn Fn()> = {
        let (state, scope, on_dismiss_request) = (state.clone(), scope.clone(), on_dismiss_request.clone());
        Rc::new(move || {
            if state.confirm_value_change(Hidden) {
                let (s, on_dismiss_request) = (state.clone(), on_dismiss_request.clone());
                launch_invoking_on_completion(&scope, state.hide(), move || {
                    if !s.is_visible() {
                        on_dismiss_request();
                    }
                });
            }
        })
    };

    let nested_scroll = if gestures_enabled {
        let fling = modal_bottom_sheet_fling_behavior.clone();
        let s = state.clone();
        let connection = remember_keyed(state.clone(), move || {
            Rc::new(ConsumeSwipeWithinBottomSheetBoundsNestedScrollConnection(s, Orientation::Vertical, fling))
                as Rc<dyn NestedScrollConnection>
        });
        named_args!(Modifier.nested_scroll(connection))
    } else {
        Modifier
    };
    let anchors_state = state.clone();
    let semantics_title = bottom_sheet_pane_title.clone();
    let surface_modifier = named_args!(modifier.width_in(max = max_width))
        .fill_max_width()
        .then(nested_scroll)
        .draggable_anchors(state.anchored_draggable_state(), Orientation::Vertical, move |sheet_size, constraints| {
            let state = &anchors_state;
            let full_height = constraints.max_height as f32;
            let new_anchors = DraggableAnchors(|a: &mut DraggableAnchorsConfig<SheetValue>| {
                a.at(Hidden, full_height);
                if sheet_size.height as f32 > (full_height / 2.0) && !state.skip_partially_expanded() {
                    a.at(PartiallyExpanded, full_height / 2.0);
                }
                if sheet_size.height != 0 {
                    a.at(Expanded, 0.0f32.max(full_height - sheet_size.height as f32));
                }
            });
            let new_target = match state.target_value() {
                Hidden => Hidden,
                PartiallyExpanded => {
                    if new_anchors.has_position_for(&PartiallyExpanded) {
                        PartiallyExpanded
                    } else if new_anchors.has_position_for(&Expanded) {
                        Expanded
                    } else {
                        Hidden
                    }
                }
                Expanded => {
                    if new_anchors.has_position_for(&Expanded) {
                        Expanded
                    } else {
                        Hidden
                    }
                }
            };
            (Arc::new(new_anchors) as Arc<dyn DraggableAnchors<SheetValue>>, new_target)
        });
    let surface_modifier = named_args!(surface_modifier.anchored_draggable(
        state.anchored_draggable_state(),
        Orientation::Vertical,
        enabled = gestures_enabled && state.current_value() != Hidden,
        fling_behavior = Some(modal_bottom_sheet_fling_behavior),
    ))
    .semantics_with(false, move |c| {
        c.set_pane_title(semantics_title.clone());
        c.set_traversal_index(0.0);
    })
    .sheet_predictive_back_scaling(state.clone(), predictive_back_progress)
    // Scale up the Surface vertically in case the sheet's offset overflows below the min anchor. This is done to
    // avoid showing a gap when the sheet opens and bounces when it's applied with a bouncy motion. Note that the
    // content inside the Surface is scaled back down to maintain its aspect ratio (see below).
    .vertical_scale_up(state.clone());

    let column_state = state.clone();
    Surface(
        modifier = surface_modifier,
        shape = shape,
        color = container_color,
        content_color = Some(content_color),
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        content = move || {
            let state = column_state.clone();
            Column(
                modifier = Modifier
                    .fill_max_width()
                    .window_insets_padding(content_window_insets())
                    .content_predictive_back_scaling(predictive_back_progress)
                    // Scale the content down in case the sheet offset overflows below the min anchor. The wrapping
                    // Surface is scaled up, so this is done to maintain the content's aspect ratio.
                    .vertical_scale_down(state.clone()),
                content = || {
                    if let Some(drag_handle) = drag_handle.clone() {
                        let collapse_action_label = get_string(Strings::BottomSheetPartialExpandDescription);
                        let dismiss_action_label = get_string(Strings::BottomSheetDismissDescription);
                        let expand_action_label = get_string(Strings::BottomSheetExpandDescription);
                        let (click_state, click_scope, click_dismiss) =
                            (state.clone(), scope.clone(), animate_to_dismiss.clone());
                        let (sem_state, sem_scope, sem_dismiss) =
                            (state.clone(), scope.clone(), animate_to_dismiss.clone());
                        DragHandleWithTooltip(
                            modifier = Modifier
                                .clickable(move || match click_state.current_value() {
                                    Expanded => click_dismiss(),
                                    PartiallyExpanded => {
                                        let s = click_state.clone();
                                        named_args!(click_scope.launch(async move { s.expand().await }));
                                    }
                                    _ => {
                                        let s = click_state.clone();
                                        named_args!(click_scope.launch(async move { s.show().await }));
                                    }
                                })
                                .semantics_with(true, move |c| {
                                    // Provides semantics to interact with the bottomsheet based on its current
                                    // value.
                                    if gestures_enabled {
                                        let state = &sem_state;
                                        let dismiss = sem_dismiss.clone();
                                        named_args!(c.dismiss(label = Some(dismiss_action_label.clone()), move || {
                                            dismiss();
                                            true
                                        }));
                                        if state.current_value() == PartiallyExpanded {
                                            let (s, scope) = (state.clone(), sem_scope.clone());
                                            named_args!(c.expand(
                                                label = Some(expand_action_label.clone()),
                                                move || {
                                                    if s.confirm_value_change(Expanded) {
                                                        let s = s.clone();
                                                        named_args!(scope.launch(async move { s.expand().await }));
                                                    }
                                                    true
                                                }
                                            ));
                                        } else if state.has_partially_expanded_state() {
                                            let (s, scope) = (state.clone(), sem_scope.clone());
                                            named_args!(c.collapse(
                                                label = Some(collapse_action_label.clone()),
                                                move || {
                                                    if s.confirm_value_change(PartiallyExpanded) {
                                                        let s = s.clone();
                                                        named_args!(
                                                            scope.launch(async move { s.partial_expand().await })
                                                        );
                                                    }
                                                    true
                                                }
                                            ));
                                        }
                                    }
                                }),
                            content = move || drag_handle(),
                        );
                    }
                    content();
                },
            );
        },
    );
}

/// The theme's default spatial spec, kept while the theme's motion scheme is the same (Compose's scheme gives the
/// same spec object each time; KompozR's scheme makes a new handle of the same spec, which would make the
/// anchored draggable's fling behavior anew on each composition).
#[track_caller]
pub(crate) fn remembered_default_spatial_spec() -> Arc<dyn AnimationSpec<f32>> {
    kompozable_block!({
        let motion_scheme = MaterialTheme::motion_scheme();
        remember_keyed(motion_scheme.clone(), move || {
            motion_scheme.default_spatial_spec::<f32>() as Arc<dyn AnimationSpec<f32>>
        })
    })
}

/// Kotlin's `{ _ -> state.positionalThreshold.invoke() }`, memoized by the state as Compose's compiler does.
#[track_caller]
pub(crate) fn remembered_positional_threshold(state: &SheetState) -> Rc<dyn Fn(f32) -> f32> {
    let s = state.clone();
    kompozable_block!(remember_keyed(state.clone(), move || {
        Rc::new(move |_: f32| (s.positional_threshold())()) as Rc<dyn Fn(f32) -> f32>
    }))
}

/// A fling behavior by its instance (a `remember` key).
#[derive(Clone)]
pub(crate) struct FlingKey(pub(crate) Rc<dyn TargetedFlingBehavior>);

impl PartialEq for FlingKey {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The modal sheet's fling (Compose's `object : FlingBehavior` in BottomSheetImpl): the velocity within the
/// system's maximum, damped near the hidden anchor (but kept at least the velocity threshold if it was), flung by
/// the anchored draggable's fling; `on_dismiss_request` called if it ends hidden (also when cancelled).
struct ModalBottomSheetFlingBehavior {
    anchored_draggable_fling_behavior: Rc<dyn TargetedFlingBehavior>,
    state: SheetState,
    maximum_fling_velocity: f32,
    density: Density,
    on_dismiss_request: Rc<dyn Fn()>,
}

impl PartialEq for ModalBottomSheetFlingBehavior {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl FlingBehavior for ModalBottomSheetFlingBehavior {
    fn perform_fling(&self, scope: Rc<dyn ScrollScope>, initial_velocity: f32) -> LocalFuture<'_, f32> {
        Box::pin(async move {
            let max_system_velocity = self.maximum_fling_velocity;
            let mut safe_velocity = coerce_in(initial_velocity, -max_system_velocity, max_system_velocity);

            let anchors = self.state.anchored_draggable_state().anchors();
            if safe_velocity > 0.0 && anchors.has_position_for(&Hidden) {
                let hidden_anchor = anchors.position_of(&Hidden);
                let current_offset = self.state.require_offset();
                let distance_to_floor = kt_max(0.0, hidden_anchor - current_offset);

                let dampening_zone = self.density.to_px(BottomSheetDefaults::BoundaryDampeningZone);
                if distance_to_floor < dampening_zone {
                    let factor = distance_to_floor / dampening_zone;
                    safe_velocity *= factor;

                    // Ensure previously valid velocities (above velocityThresholdPx) shrink at most to
                    // velocityThresholdPx to maintain a valid fling.
                    let velocity_threshold_px = self.density.to_px(BottomSheetDefaults::VelocityThreshold);
                    if initial_velocity >= velocity_threshold_px {
                        safe_velocity = kt_max(safe_velocity, velocity_threshold_px);
                    }
                }
            }

            // (Kotlin's `try { … } finally { if (!state.isVisible) onDismissRequest() }`)
            let _finally = DismissIfHidden(self.state.clone(), self.on_dismiss_request.clone());
            self.anchored_draggable_fling_behavior.perform_fling(scope, safe_velocity).await
        })
    }
}

/// Calls `on_dismiss_request` when dropped (the fling done or cancelled) if the sheet isn't visible.
struct DismissIfHidden(SheetState, Rc<dyn Fn()>);

impl Drop for DismissIfHidden {
    fn drop(&mut self) {
        if !self.0.is_visible() {
            (self.1)();
        }
    }
}

/// Kotlin's `coerceIn` of a float.
fn coerce_in(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

/// Kotlin's `max` of floats (Java's Math.max: NaN wins).
fn kt_max(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() { f32::NAN } else { a.max(b) }
}

/// Kotlin's `min` of floats (Java's Math.min: NaN wins).
fn kt_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() { f32::NAN } else { a.min(b) }
}

// ---------------------------------------------------------------------------------------------------------------
// SheetState

/// The state of a sheet, as a [`ModalBottomSheet`](crate::ModalBottomSheet)'s or a
/// [`BottomSheetScaffold`](crate::BottomSheetScaffold)'s (Compose's `SheetState`): where it is and where it is
/// going, and the animations between its values. A cheap handle: clones are the same state (equal when they are).
#[derive(Clone)]
pub struct SheetState {
    inner: Rc<SheetStateInner>,
}

struct SheetStateInner {
    enabled_values: KtSet<SheetValue>,
    positional_threshold: Rc<dyn Fn() -> f32>,
    // (Compose's internal `velocityThreshold`: kept, read by nothing in the library, as in Compose)
    #[allow(dead_code)]
    velocity_threshold: Rc<dyn Fn() -> f32>,
    confirm_value_change: Rc<dyn Fn(SheetValue) -> bool>,
    target_value: DerivedState<SheetValue>,
    anchored_draggable_motion_spec: RefCell<Arc<dyn AnimationSpec<f32>>>,
    anchored_draggable_state: AnchoredDraggableState<SheetValue>,
    show_motion_spec: RefCell<Arc<dyn FiniteAnimationSpec<f32>>>,
    hide_motion_spec: RefCell<Arc<dyn FiniteAnimationSpec<f32>>>,
}

impl PartialEq for SheetState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for SheetState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SheetState(currentValue={:?}, offset={})", self.current_value(), self.offset())
    }
}

/// A sheet's state (Compose's `SheetState(enabledValues, positionalThreshold, velocityThreshold, initialValue =
/// Hidden, confirmValueChange = { true })`, material3 1.5.0-alpha20): `enabled_values` the values it can settle at
/// (each one it has an anchor for, when its sheet's size allows); `positional_threshold` (px) how far from the start
/// of a move it must be dragged to settle on the next value, `velocity_threshold` (px/s) how fast a fling must be to
/// go on to the next one anyway; `initial_value` where it starts; `confirm_value_change` may veto a change.
///
/// Panics (Kotlin's IllegalArgumentException) when `enabled_values` lacks [`Expanded`](SheetValue::Expanded) or
/// `initial_value`.
#[named(type)]
pub fn SheetState(
    enabled_values: KtSet<SheetValue>,
    positional_threshold: impl Fn() -> f32 + 'static,
    velocity_threshold: impl Fn() -> f32 + 'static,
    #[default(Hidden)] initial_value: SheetValue,
    #[default(|_| true)] confirm_value_change: impl Fn(SheetValue) -> bool + 'static,
) -> SheetState {
    SheetState::new(
        enabled_values,
        Rc::new(positional_threshold),
        Rc::new(velocity_threshold),
        initial_value,
        Rc::new(confirm_value_change),
    )
}

/// A sheet's state of the values it skips (Compose's deprecated `SheetState(skipPartiallyExpanded,
/// positionalThreshold, velocityThreshold, initialValue = Hidden, confirmValueChange = { true }, skipHiddenState =
/// false)`, material3 1.5.0-alpha20): [`SheetState()`](fn@SheetState) of [`Expanded`](SheetValue::Expanded), and
/// [`PartiallyExpanded`](SheetValue::PartiallyExpanded) unless `skip_partially_expanded`, and
/// [`Hidden`](SheetValue::Hidden) unless `skip_hidden_state`.
///
/// Kotlin: `SheetState(skipPartiallyExpanded: Boolean, positionalThreshold: () -> Float, velocityThreshold: () -> Float, initialValue: SheetValue = Hidden, confirmValueChange: (SheetValue) -> Boolean = { true }, skipHiddenState: Boolean = false)`
#[deprecated(note = "Use the primary constructor that takes a set of enabled values.")]
#[named]
pub fn SheetStateSkipping(
    skip_partially_expanded: bool,
    positional_threshold: impl Fn() -> f32 + 'static,
    velocity_threshold: impl Fn() -> f32 + 'static,
    #[default(Hidden)] initial_value: SheetValue,
    #[default(|_| true)] confirm_value_change: impl Fn(SheetValue) -> bool + 'static,
    #[default(false)] skip_hidden_state: bool,
) -> SheetState {
    SheetState::new(
        enabled_values_skipping(skip_partially_expanded, skip_hidden_state),
        Rc::new(positional_threshold),
        Rc::new(velocity_threshold),
        initial_value,
        Rc::new(confirm_value_change),
    )
}

/// The values of a sheet skipping what it is told to (Kotlin's `buildSet { add(Expanded); if
/// (!skipPartiallyExpanded) add(PartiallyExpanded); if (!skipHiddenState) add(Hidden) }`).
pub(crate) fn enabled_values_skipping(skip_partially_expanded: bool, skip_hidden_state: bool) -> KtSet<SheetValue> {
    let mut values = KtSet::new();
    values.add(Expanded);
    if !skip_partially_expanded {
        values.add(PartiallyExpanded);
    }
    if !skip_hidden_state {
        values.add(Hidden);
    }
    values
}

/// Every value of a sheet (Kotlin's default `setOf(Hidden, PartiallyExpanded, Expanded)`).
pub(crate) fn all_sheet_values() -> KtSet<SheetValue> {
    set_of([Hidden, PartiallyExpanded, Expanded])
}

impl SheetState {
    /// [`SheetState()`](fn@SheetState) of shared closures.
    pub(crate) fn new(
        enabled_values: KtSet<SheetValue>,
        positional_threshold: Rc<dyn Fn() -> f32>,
        velocity_threshold: Rc<dyn Fn() -> f32>,
        initial_value: SheetValue,
        confirm_value_change: Rc<dyn Fn(SheetValue) -> bool>,
    ) -> SheetState {
        if !enabled_values.contains(Expanded) {
            panic_kt("IllegalArgumentException", "Expanded must be one of the enabled values.");
        }
        if !enabled_values.contains(initial_value) {
            panic_kt("IllegalArgumentException", "The initial value must be one of the enabled values.");
        }
        let confirm = confirm_value_change.clone();
        #[allow(deprecated)]
        let anchored_draggable_state =
            anchored_draggable_state_confirming(initial_value, move |value: &SheetValue| confirm(*value));
        // TODO(b/477969920): Remove forked targetValue logic when foundation dependencies are updated.
        let target_value = {
            let anchored = anchored_draggable_state.clone();
            derived_state_of(move || {
                // AnchoredDraggableState does not expose the dragTarget, but isAnimationRunning returns whether
                // AnchoredDraggableState.dragTarget is null. If it's not, we can use the targetValue; otherwise we
                // apply the calculation fix.
                if anchored.is_animation_running() {
                    anchored.target_value()
                } else {
                    calculate_target_value_with_fix(&anchored, anchored.offset())
                }
            })
        };
        SheetState {
            inner: Rc::new(SheetStateInner {
                enabled_values,
                positional_threshold,
                velocity_threshold,
                confirm_value_change,
                target_value,
                anchored_draggable_motion_spec: RefCell::new(BottomSheetAnimationSpec()),
                anchored_draggable_state,
                show_motion_spec: RefCell::new(Arc::new(snap::<f32>())),
                hide_motion_spec: RefCell::new(Arc::new(snap::<f32>())),
            }),
        }
    }

    /// The value it has settled at (Compose's `currentValue`): before a drag or animation ends, the one it
    /// started from. Note: it maps to the anchored draggable's settled value, its current value being the anchor
    /// the touch is closest to.
    pub fn current_value(&self) -> SheetValue {
        self.inner.anchored_draggable_state.settled_value()
    }

    /// The value it is going to (Compose's `targetValue`): the animation's target while one runs, else the anchor
    /// closest to where it is (the current value's while it is there); its current value while not moving.
    pub fn target_value(&self) -> SheetValue {
        self.inner.target_value.get()
    }

    /// Whether it is visible (Compose's `isVisible`): its anchored state isn't at hidden.
    pub fn is_visible(&self) -> bool {
        self.inner.anchored_draggable_state.current_value() != Hidden
    }

    /// Whether an animation runs (Compose's `isAnimationRunning`).
    pub fn is_animation_running(&self) -> bool {
        self.inner.anchored_draggable_state.is_animation_running()
    }

    /// Its offset, px from the top of its room (Compose's `requireOffset`). Panics (Kotlin's
    /// IllegalStateException) before it was laid out.
    pub fn require_offset(&self) -> f32 {
        self.inner.anchored_draggable_state.require_offset()
    }

    /// Whether it has an expanded value (Compose's `hasExpandedState`).
    pub fn has_expanded_state(&self) -> bool {
        self.inner.anchored_draggable_state.anchors().has_position_for(&Expanded)
    }

    /// Whether it has a partially expanded value (Compose's `hasPartiallyExpandedState`).
    pub fn has_partially_expanded_state(&self) -> bool {
        self.inner.anchored_draggable_state.anchors().has_position_for(&PartiallyExpanded)
    }

    /// Expands it, animating by its show motion, if confirmed (Compose's suspending `expand`).
    pub fn expand(&self) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            if state.confirm_value_change(Expanded) {
                let spec = state.show_motion_spec();
                state.animate_to(Expanded, spec).await;
            }
        }
    }

    /// Takes it half way, animating by its hide motion, if confirmed (Compose's suspending `partialExpand`). Panics
    /// (Kotlin's IllegalStateException) when it skips that value.
    pub fn partial_expand(&self) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            if state.skip_partially_expanded() {
                panic_kt(
                    "IllegalStateException",
                    "Attempted to animate to partial expanded when skipPartiallyExpanded was enabled. Set \
                     skipPartiallyExpanded to false to use this function.",
                );
            }
            if state.confirm_value_change(PartiallyExpanded) {
                let spec = state.hide_motion_spec();
                state.animate_to(PartiallyExpanded, spec).await;
            }
        }
    }

    /// Shows it, half way if it has that value, else expanded, animating by its show motion, if confirmed
    /// (Compose's suspending `show`).
    pub fn show(&self) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            let target_value = if state.has_partially_expanded_state() { PartiallyExpanded } else { Expanded };
            if state.confirm_value_change(target_value) {
                let spec = state.show_motion_spec();
                state.animate_to(target_value, spec).await;
            }
        }
    }

    /// Hides it, animating by its hide motion, if confirmed (Compose's suspending `hide`). Panics (Kotlin's
    /// IllegalStateException) when it skips that value.
    pub fn hide(&self) -> impl Future<Output = ()> + 'static {
        let state = self.clone();
        async move {
            if state.skip_hidden_state() {
                panic_kt(
                    "IllegalStateException",
                    "Attempted to animate to hidden when skipHiddenState was enabled. Set skipHiddenState to false \
                     to use this function.",
                );
            }
            if state.confirm_value_change(Hidden) {
                let spec = state.hide_motion_spec();
                state.animate_to(Hidden, spec).await;
            }
        }
    }

    /// Animates it to `target_value` along `animation_spec` (Compose's internal `animateTo`).
    pub(crate) fn animate_to(
        &self,
        target_value: SheetValue,
        animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    ) -> impl Future<Output = ()> + 'static {
        named_args!(
            self.inner
                .anchored_draggable_state
                .animate_to(target_value, animation_spec = animation_spec as Arc<dyn AnimationSpec<f32>>)
        )
    }

    /// Snaps it to `target_value` (Compose's internal `snapTo`; used by nothing in the library, as in Compose).
    #[allow(dead_code)]
    pub(crate) fn snap_to(&self, target_value: SheetValue) -> impl Future<Output = ()> + 'static {
        self.inner.anchored_draggable_state.snap_to(target_value)
    }

    /// The spec its anchored draggable settles by (Compose's internal `anchoredDraggableMotionSpec`; set by the
    /// sheets, read by nothing in the library, as in Compose).
    #[allow(dead_code)]
    pub(crate) fn anchored_draggable_motion_spec(&self) -> Arc<dyn AnimationSpec<f32>> {
        self.inner.anchored_draggable_motion_spec.borrow().clone()
    }

    pub(crate) fn set_anchored_draggable_motion_spec(&self, spec: Arc<dyn FiniteAnimationSpec<f32>>) {
        *self.inner.anchored_draggable_motion_spec.borrow_mut() = spec;
    }

    /// Its anchored draggable state (Compose's internal `anchoredDraggableState`).
    pub(crate) fn anchored_draggable_state(&self) -> AnchoredDraggableState<SheetValue> {
        self.inner.anchored_draggable_state.clone()
    }

    /// Its offset moved by `delta`, within its anchors (Compose's internal `newOffsetForDelta`).
    pub(crate) fn new_offset_for_delta(&self, delta: f32) -> f32 {
        let offset = self.offset();
        let anchors = self.inner.anchored_draggable_state.anchors();
        let (min, max) = (anchors.min_position(), anchors.max_position());
        let v = (if offset.is_nan() { 0.0 } else { offset }) + delta;
        if min > max {
            panic_kt(
                "IllegalArgumentException",
                "Cannot coerce value to an empty range: maximum is less than minimum.",
            );
        }
        coerce_in(v, min, max)
    }

    /// Flings it by `fling_behavior` from `initial_velocity` in an anchored drag of its own (Compose's internal
    /// `anchoredDrag`): the velocity the fling used.
    pub(crate) fn anchored_drag(
        &self,
        fling_behavior: Rc<dyn FlingBehavior>,
        initial_velocity: f32,
    ) -> impl Future<Output = f32> + 'static {
        let consumed_velocity = Rc::new(Cell::new(0.0f32));
        let (state, consumed) = (self.clone(), consumed_velocity.clone());
        let drag = named_args!(self.inner.anchored_draggable_state.anchored_drag(move |drag_scope, _anchors| {
            let (state, fling_behavior, consumed) = (state.clone(), fling_behavior.clone(), consumed.clone());
            async move {
                let scroll_scope: Rc<dyn ScrollScope> = Rc::new(SheetScrollScope { state, drag_scope });
                consumed.set(fling_behavior.perform_fling(scroll_scope, initial_velocity).await);
            }
        }));
        async move {
            drag.await;
            consumed_velocity.get()
        }
    }

    /// Its offset, NaN before it was laid out (Compose's internal `offset`).
    pub(crate) fn offset(&self) -> f32 {
        self.inner.anchored_draggable_state.offset()
    }

    /// The motion it shows by (Compose's internal `showMotionSpec`, a snap until a sheet sets the theme's).
    pub(crate) fn show_motion_spec(&self) -> Arc<dyn FiniteAnimationSpec<f32>> {
        self.inner.show_motion_spec.borrow().clone()
    }

    pub(crate) fn set_show_motion_spec(&self, spec: Arc<dyn FiniteAnimationSpec<f32>>) {
        *self.inner.show_motion_spec.borrow_mut() = spec;
    }

    /// The motion it hides by (Compose's internal `hideMotionSpec`).
    pub(crate) fn hide_motion_spec(&self) -> Arc<dyn FiniteAnimationSpec<f32>> {
        self.inner.hide_motion_spec.borrow().clone()
    }

    pub(crate) fn set_hide_motion_spec(&self, spec: Arc<dyn FiniteAnimationSpec<f32>>) {
        *self.inner.hide_motion_spec.borrow_mut() = spec;
    }

    /// Whether it skips the partially expanded value: not one of its enabled values (Compose's internal
    /// `skipPartiallyExpanded`, material3 1.5.0-alpha20).
    pub(crate) fn skip_partially_expanded(&self) -> bool {
        !self.inner.enabled_values.contains(PartiallyExpanded)
    }

    /// Whether it skips the hidden value: not one of its enabled values (Compose's internal `skipHiddenState`,
    /// material3 1.5.0-alpha20).
    pub(crate) fn skip_hidden_state(&self) -> bool {
        !self.inner.enabled_values.contains(Hidden)
    }

    /// Its positional threshold, px (Compose's internal `positionalThreshold`).
    pub(crate) fn positional_threshold(&self) -> Rc<dyn Fn() -> f32> {
        self.inner.positional_threshold.clone()
    }

    /// Whether it may change to `value` (Compose's internal `confirmValueChange`).
    pub(crate) fn confirm_value_change(&self, value: SheetValue) -> bool {
        (self.inner.confirm_value_change)(value)
    }

    /// The default saver of a sheet's state (Compose's `SheetState.Saver(enabledValues, positionalThreshold,
    /// velocityThreshold, confirmValueChange)`, material3 1.5.0-alpha20): its current value saved, a state of it
    /// restored.
    pub fn Saver(
        enabled_values: KtSet<SheetValue>,
        positional_threshold: impl Fn() -> f32 + 'static,
        velocity_threshold: impl Fn() -> f32 + 'static,
        confirm_value_change: impl Fn(SheetValue) -> bool + 'static,
    ) -> Saver<SheetState> {
        SheetState::saver_of(
            enabled_values,
            Rc::new(positional_threshold),
            Rc::new(velocity_threshold),
            Rc::new(confirm_value_change),
        )
    }

    /// The default saver of a sheet's state of the values it skips (Compose's deprecated
    /// `SheetState.Saver(skipPartiallyExpanded, positionalThreshold, velocityThreshold, confirmValueChange,
    /// skipHiddenState)`, material3 1.5.0-alpha20): [`Saver`](Self::Saver) of the values
    /// [`SheetStateSkipping`](fn@SheetStateSkipping) has.
    ///
    /// Kotlin: `Saver(skipPartiallyExpanded: Boolean, positionalThreshold: () -> Float, velocityThreshold: () -> Float, confirmValueChange: (SheetValue) -> Boolean, skipHiddenState: Boolean): Saver<SheetState, SheetValue>`
    #[deprecated(note = "Use the Saver that takes a set of enabled values.")]
    pub fn SaverSkipping(
        skip_partially_expanded: bool,
        positional_threshold: impl Fn() -> f32 + 'static,
        velocity_threshold: impl Fn() -> f32 + 'static,
        confirm_value_change: impl Fn(SheetValue) -> bool + 'static,
        skip_hidden_state: bool,
    ) -> Saver<SheetState> {
        SheetState::Saver(
            enabled_values_skipping(skip_partially_expanded, skip_hidden_state),
            positional_threshold,
            velocity_threshold,
            confirm_value_change,
        )
    }

    /// [`Saver`](Self::Saver) of shared closures.
    pub(crate) fn saver_of(
        enabled_values: KtSet<SheetValue>,
        positional_threshold: Rc<dyn Fn() -> f32>,
        velocity_threshold: Rc<dyn Fn() -> f32>,
        confirm_value_change: Rc<dyn Fn(SheetValue) -> bool>,
    ) -> Saver<SheetState> {
        Saver::new(
            |state: &SheetState| state.current_value().save(),
            move |saved| {
                let saved_value = SheetValue::restore(saved)?;
                Some(SheetState::new(
                    enabled_values.clone(),
                    positional_threshold.clone(),
                    velocity_threshold.clone(),
                    saved_value,
                    confirm_value_change.clone(),
                ))
            },
        )
    }
}

/// Compose's private `calculateTargetValueWithFix`.
fn calculate_target_value_with_fix(state: &AnchoredDraggableState<SheetValue>, current_offset: f32) -> SheetValue {
    let current_value = state.settled_value();
    if !current_offset.is_nan() {
        // DraggableAnchors allows multiple anchors with the same offsets. If the offset is already equal to the
        // currentValue's offset, this anchor gets priority.
        let anchors = state.anchors();
        let current_value_offset = anchors.position_of(&current_value);
        if current_value_offset.is_nan() || current_offset == current_value_offset {
            current_value
        } else {
            anchors.closest_anchor(current_offset).unwrap_or(current_value)
        }
    } else {
        current_value
    }
}

/// The scroll scope of [`SheetState::anchored_drag`]'s fling (Compose's `object : ScrollScope`): a scroll moves
/// the sheet within its anchors.
struct SheetScrollScope {
    state: SheetState,
    drag_scope: Rc<dyn AnchoredDragScope>,
}

impl ScrollScope for SheetScrollScope {
    fn scroll_by(&self, pixels: f32) -> f32 {
        let new_offset = self.state.new_offset_for_delta(pixels);
        let consumed = new_offset - self.state.offset();
        self.drag_scope.drag_to(new_offset, 0.0);
        consumed
    }
}

/// The values of a [`SheetState`] (Compose's enum `SheetValue`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SheetValue {
    /// The sheet is not visible.
    Hidden,
    /// The sheet is visible at full height.
    Expanded,
    /// The sheet is partially visible.
    PartiallyExpanded,
}

impl SheetValue {
    /// Its values in order (Kotlin's `entries`).
    pub const ENTRIES: [SheetValue; 3] = [Hidden, Expanded, PartiallyExpanded];
}

/// Its name (Kotlin's enum `toString`).
impl fmt::Display for SheetValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// Saved as its ordinal (Kotlin's enums are Serializable).
impl Saveable for SheetValue {
    fn save(&self) -> SavedValue {
        SavedValue::Int(*self as i64)
    }

    fn restore(saved: &SavedValue) -> Option<Self> {
        let ordinal = usize::try_from(i64::restore(saved)?).ok()?;
        SheetValue::ENTRIES.get(ordinal).copied()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// BottomSheetDefaults

/// The defaults of [`ModalBottomSheet`](crate::ModalBottomSheet) and
/// [`BottomSheetScaffold`](crate::BottomSheetScaffold) (Compose's `object BottomSheetDefaults`; a unit value:
/// `BottomSheetDefaults.DragHandle()`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BottomSheetDefaults;

impl BottomSheetDefaults {
    /// The shape of a hidden bottom sheet (Compose's `HiddenShape`): no corners.
    pub fn HiddenShape() -> Arc<dyn Shape> {
        SheetBottomTokens::DockedMinimizedContainerShape.value()
    }

    /// The shape of a partially expanded or expanded bottom sheet (Compose's `ExpandedShape`): its top corners
    /// extra large.
    pub fn ExpandedShape() -> Arc<dyn Shape> {
        SheetBottomTokens::DockedContainerShape.value()
    }

    /// A bottom sheet's container color (Compose's `ContainerColor`): the scheme's low surface container.
    pub fn ContainerColor() -> Color {
        SheetBottomTokens::DockedContainerColor.value()
    }

    /// A bottom sheet's elevation (Compose's `Elevation`): 1 dp.
    pub const Elevation: Dp = SheetBottomTokens::DockedModalContainerElevation;

    /// The color of the scrim over the content behind (Compose's `ScrimColor`): the scheme's scrim at 32%.
    pub fn ScrimColor() -> Color {
        named_args!(ScrimTokens::ContainerColor.value().copy(alpha = ScrimTokens::ContainerOpacity))
    }

    /// The peek height of a [`BottomSheetScaffold`](crate::BottomSheetScaffold)'s sheet (Compose's
    /// `SheetPeekHeight`): 56 dp.
    pub const SheetPeekHeight: Dp = Dp(56.0);

    /// The maximum width of a sheet (Compose's `SheetMaxWidth`): 640 dp.
    pub const SheetMaxWidth: Dp = Dp(640.0);

    /// The insets a [`ModalBottomSheet`](crate::ModalBottomSheet)'s content uses and consumes (Compose's
    /// deprecated `windowInsets`): the safe drawing ones at the top and bottom.
    #[deprecated(note = "Renamed as modalWindowInsets.")]
    pub fn window_insets() -> WindowInsets {
        WindowInsets::safe_drawing().only(WindowInsetsSides::Bottom + WindowInsetsSides::Top)
    }

    /// The insets a [`BottomSheet`]'s content uses and consumes (Compose's `standardWindowInsets`): the safe drawing
    /// ones at the bottom.
    pub fn standard_window_insets() -> WindowInsets {
        WindowInsets::safe_drawing().only(WindowInsetsSides::Bottom)
    }

    /// The insets a [`ModalBottomSheet`](crate::ModalBottomSheet)'s content uses and consumes (Compose's
    /// `modalWindowInsets`): the safe drawing ones at the top and bottom.
    pub fn modal_window_insets() -> WindowInsets {
        WindowInsets::safe_drawing().only(WindowInsetsSides::Bottom + WindowInsetsSides::Top)
    }

    /// Compose's internal `PositionalThreshold`.
    pub(crate) const PositionalThreshold: Dp = Dp(56.0);

    /// Compose's internal `VelocityThreshold`.
    pub(crate) const VelocityThreshold: Dp = Dp(125.0);

    /// The distance above the hidden anchor where a fling towards it is damped, so a fling at the bottom doesn't
    /// hide the sheet at once (Compose's internal `BoundaryDampeningZone`).
    pub(crate) const BoundaryDampeningZone: Dp = Dp(125.0);
}

#[named]
impl BottomSheetDefaults {
    /// The marker on top of a bottom sheet telling it can be dragged (Compose's `DragHandle`): a `width` ×
    /// `height` [`Surface`] of `shape` (None: the theme's extra large shape) in `color`, 22 dp above and below it,
    /// described as the drag handle.
    #[track_caller]
    pub fn DragHandle(
        &self,
        #[default(Modifier)] modifier: Modifier,
        #[default(SheetBottomTokens::DockedDragHandleWidth)] width: Dp,
        #[default(SheetBottomTokens::DockedDragHandleHeight)] height: Dp,
        #[default(None)] shape: Option<Arc<dyn Shape>>,
        #[default(SheetBottomTokens::DockedDragHandleColor.value())] color: Color,
    ) {
        kompozable_block!({
            DragHandleImpl(modifier, width, height, shape, color);
        })
    }
}

/// [`BottomSheetDefaults::DragHandle`]'s body.
#[kompozable]
fn DragHandleImpl(modifier: Modifier, width: Dp, height: Dp, shape: Option<Arc<dyn Shape>>, color: Color) {
    let shape = shape.unwrap_or_else(|| Arc::new(MaterialTheme::shapes().extra_large));
    let drag_handle_description = get_string(Strings::BottomSheetDragHandleDescription);
    Surface(
        modifier = named_args!(modifier.padding_hv(vertical = DragHandleVerticalPadding))
            .semantics_with(false, move |c| c.set_content_description(drag_handle_description.clone())),
        color = color,
        shape = shape,
        content = || Box(modifier = Modifier.size_wh(width, height)),
    );
}

/// The drag handle in its tooltip, centered in the sheet's width (Compose's internal `DragHandleWithTooltip`):
/// `content` in a [`TooltipBox`](crate::TooltipBox) of `modifier` above it, its plain tooltip the drag handle's
/// description.
#[kompozable]
pub(crate) fn DragHandleWithTooltip(modifier: Modifier, content: impl Fn() + 'static) {
    let drag_handle_description = get_string(Strings::BottomSheetDragHandleDescription);
    let content: Rc<dyn Fn()> = Rc::new(content);
    Box(
        modifier = Modifier.fill_max_width(),
        content_alignment = Alignment::Center,
        content = || {
            let description = drag_handle_description.clone();
            let content = content.clone();
            TooltipBox(
                modifier = modifier.clone(),
                position_provider = TooltipDefaults.remember_tooltip_position_provider(TooltipAnchorPosition::Above),
                tooltip = move |s: &dyn TooltipScope| {
                    let text = description.clone();
                    s.PlainTooltip(content = move || Text(text.clone()));
                },
                state = remember_tooltip_state(),
                content = move || content(),
            );
        },
    );
}

// ---------------------------------------------------------------------------------------------------------------
// The nested scroll connection

/// The sheet's link of the nested scroll chain (Compose's internal
/// `ConsumeSwipeWithinBottomSheetBoundsNestedScrollConnection`): a user's scroll up moves the sheet up before what
/// scrolls in it; what that leaves (either way) moves the sheet; a fling up while the sheet isn't at its top flings
/// it (all of the fling consumed), and what a fling leaves flings it on.
pub(crate) struct ConsumeSwipeWithinBottomSheetBoundsNestedScrollConnection(
    pub(crate) SheetState,
    pub(crate) Orientation,
    pub(crate) Rc<dyn FlingBehavior>,
);

impl ConsumeSwipeWithinBottomSheetBoundsNestedScrollConnection {
    fn float_to_offset(&self, v: f32) -> Offset {
        Offset::new(
            if self.1 == Orientation::Horizontal { v } else { 0.0 },
            if self.1 == Orientation::Vertical { v } else { 0.0 },
        )
    }

    fn velocity_to_float(&self, v: Velocity) -> f32 {
        if self.1 == Orientation::Horizontal { v.x } else { v.y }
    }

    fn offset_to_float(&self, v: Offset) -> f32 {
        if self.1 == Orientation::Horizontal { v.x } else { v.y }
    }
}

impl NestedScrollConnection for ConsumeSwipeWithinBottomSheetBoundsNestedScrollConnection {
    fn on_pre_scroll(&self, available: Offset, source: NestedScrollSource) -> Offset {
        let delta = self.offset_to_float(available);
        if delta < 0.0 && source == NestedScrollSource::UserInput {
            self.float_to_offset(self.0.anchored_draggable_state().dispatch_raw_delta(delta))
        } else {
            Offset::Zero
        }
    }

    fn on_post_scroll(&self, _consumed: Offset, available: Offset, source: NestedScrollSource) -> Offset {
        if source == NestedScrollSource::UserInput {
            self.float_to_offset(self.0.anchored_draggable_state().dispatch_raw_delta(self.offset_to_float(available)))
        } else {
            Offset::Zero
        }
    }

    fn on_pre_fling(&self, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            let to_fling = self.velocity_to_float(available);
            let current_offset = self.0.require_offset();
            let min_anchor = self.0.anchored_draggable_state().anchors().min_position();
            if to_fling < 0.0 && current_offset > min_anchor {
                self.0.anchored_drag(self.2.clone(), to_fling).await;
                // since we go to the anchor with tween settling, consume all for the best UX
                available
            } else {
                Velocity::Zero
            }
        })
    }

    fn on_post_fling(&self, consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            let to_fling = self.velocity_to_float(available);
            let consumed_by_anchored_draggable_fling = self.0.anchored_drag(self.2.clone(), to_fling).await;
            Velocity::new(consumed.x, consumed_by_anchored_draggable_fling)
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// rememberSheetState

/// A sheet's state for a [`BottomSheet`], a [`ModalBottomSheet`](crate::ModalBottomSheet) or a
/// [`BottomSheetScaffold`](crate::BottomSheetScaffold), saved and restored (Compose's `rememberBottomSheetState`,
/// material3 1.5.0-alpha20): at `initial_value` to start with; `enabled_values` the values it can settle at (each
/// one it gets an anchor for); `confirm_value_change` may veto a change (a new closure, by what it captures, makes a
/// new state, as a new key does in Compose).
#[kompozable]
pub fn remember_bottom_sheet_state(
    initial_value: SheetValue,
    #[default(all_sheet_values())] enabled_values: KtSet<SheetValue>,
    #[default(|_| true)]
    #[memoized]
    confirm_value_change: impl Fn(SheetValue) -> bool + 'static,
) -> SheetState {
    let id = confirm_value_change.id();
    let confirm: Rc<dyn Fn(SheetValue) -> bool> = Rc::new(confirm_value_change.into_value());
    remember_sheet_state(
        enabled_values = enabled_values,
        confirm_value_change = Memoized::new(confirm, id),
        initial_value = initial_value,
    )
}

/// A sheet's state, saved and restored by [`SheetState::Saver`], made again when `enabled_values` or
/// `confirm_value_change` (by its identity) change (Compose's internal `rememberSheetState`, material3
/// 1.5.0-alpha20); its thresholds `positional_threshold` and `velocity_threshold` in this density.
#[kompozable]
pub(crate) fn remember_sheet_state(
    #[default(all_sheet_values())] enabled_values: KtSet<SheetValue>,
    confirm_value_change: Memoized<Rc<dyn Fn(SheetValue) -> bool>>,
    #[default(Hidden)] initial_value: SheetValue,
    #[default(BottomSheetDefaults::PositionalThreshold)] positional_threshold: Dp,
    #[default(BottomSheetDefaults::VelocityThreshold)] velocity_threshold: Dp,
) -> SheetState {
    let density = LocalDensity.current();
    let positional_threshold_to_px: Rc<dyn Fn() -> f32> = Rc::new(move || density.to_px(positional_threshold));
    let velocity_threshold_to_px: Rc<dyn Fn() -> f32> = Rc::new(move || density.to_px(velocity_threshold));
    let confirm_id = confirm_value_change.id();
    let confirm_value_change = confirm_value_change.into_value();
    let saver = SheetState::saver_of(
        enabled_values.clone(),
        positional_threshold_to_px.clone(),
        velocity_threshold_to_px.clone(),
        confirm_value_change.clone(),
    );
    remember_saveable_keyed_with((enabled_values.clone(), confirm_id), &saver, move || {
        SheetState::new(
            enabled_values,
            positional_threshold_to_px,
            velocity_threshold_to_px,
            initial_value,
            confirm_value_change,
        )
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The scalings

/// The sheet's horizontal scale at a predictive back's `progress`: at most 48 dp narrower (Compose's internal
/// `GraphicsLayerScope.calculateSheetPredictiveBackScaleX`).
pub(crate) fn calculate_sheet_predictive_back_scale_x(scope: &GraphicsLayerScope, progress: f32) -> f32 {
    let width = scope.size().width;
    if width.is_nan() || width == 0.0 {
        1.0
    } else {
        1.0 - lerp(0.0, kt_min(scope.density().to_px(PredictiveBackMaxScaleXDistance), width), progress) / width
    }
}

/// The sheet's vertical scale at a predictive back's `progress`: at most 24 dp lower (Compose's internal
/// `GraphicsLayerScope.calculateSheetPredictiveBackScaleY`).
pub(crate) fn calculate_sheet_predictive_back_scale_y(scope: &GraphicsLayerScope, progress: f32) -> f32 {
    let height = scope.size().height;
    if height.is_nan() || height == 0.0 {
        1.0
    } else {
        1.0 - lerp(0.0, kt_min(scope.density().to_px(PredictiveBackMaxScaleYDistance), height), progress) / height
    }
}

/// The sheets' graphics layers (Compose's internal Modifier extensions of SheetDefaults.kt).
pub(crate) trait SheetModifiers {
    /// Scales the sheet by a predictive back's progress about its bottom at the window's (Compose's internal
    /// `Modifier.sheetPredictiveBackScaling`).
    fn sheet_predictive_back_scaling(self, sheet_state: SheetState, predictive_back_progress: f32) -> Modifier;

    /// Scales the sheet's content back along its height, so it keeps its aspect (Compose's internal
    /// `Modifier.contentPredictiveBackScaling`).
    fn content_predictive_back_scaling(self, predictive_back_progress: f32) -> Modifier;

    /// Scales the sheet up from its top by how far its offset overflows below its smallest anchor (Compose's
    /// internal `Modifier.verticalScaleUp`): a bouncy opening's overshoot shows no gap below it.
    fn vertical_scale_up(self, state: SheetState) -> Modifier;

    /// Scales the sheet's content back down as the sheet is scaled up (Compose's internal
    /// `Modifier.verticalScaleDown`).
    fn vertical_scale_down(self, state: SheetState) -> Modifier;
}

impl SheetModifiers for Modifier {
    fn sheet_predictive_back_scaling(self, sheet_state: SheetState, predictive_back_progress: f32) -> Modifier {
        self.graphics_layer_block(move |scope| {
            let sheet_offset = sheet_state.anchored_draggable_state().offset();
            let sheet_height = scope.size().height;
            if !sheet_offset.is_nan() && !sheet_height.is_nan() && sheet_height != 0.0 {
                scope.scale_x = calculate_sheet_predictive_back_scale_x(scope, predictive_back_progress);
                scope.scale_y = calculate_sheet_predictive_back_scale_y(scope, predictive_back_progress);
                scope.transform_origin = TransformOrigin::new(0.5, (sheet_offset + sheet_height) / sheet_height);
            }
        })
    }

    fn content_predictive_back_scaling(self, predictive_back_progress: f32) -> Modifier {
        self.graphics_layer_block(move |scope| {
            let predictive_back_scale_x = calculate_sheet_predictive_back_scale_x(scope, predictive_back_progress);
            let predictive_back_scale_y = calculate_sheet_predictive_back_scale_y(scope, predictive_back_progress);
            scope.scale_y =
                if predictive_back_scale_y != 0.0 { predictive_back_scale_x / predictive_back_scale_y } else { 1.0 };
            scope.transform_origin = PredictiveBackChildTransformOrigin;
        })
    }

    fn vertical_scale_up(self, state: SheetState) -> Modifier {
        self.graphics_layer_block(move |scope| {
            let offset = state.anchored_draggable_state().offset();
            let anchor = state.anchored_draggable_state().anchors().min_position();
            let overflow = if offset < anchor { anchor - offset } else { 0.0 };
            let height = scope.size().height;
            scope.scale_y = if overflow > 0.0 { (height + overflow) / height } else { 1.0 };
            scope.transform_origin = TransformOrigin::new(0.5, 0.0);
        })
    }

    fn vertical_scale_down(self, state: SheetState) -> Modifier {
        self.graphics_layer_block(move |scope| {
            let offset = state.anchored_draggable_state().offset();
            let anchor = state.anchored_draggable_state().anchors().min_position();
            let overflow = if offset < anchor { anchor - offset } else { 0.0 };
            let height = scope.size().height;
            scope.scale_y = if overflow > 0.0 { 1.0 / ((height + overflow) / height) } else { 1.0 };
            scope.transform_origin = TransformOrigin::new(0.5, 0.0);
        })
    }
}

/// The spec [`SheetState`] settles by until a sheet sets the theme's (Compose's internal
/// `BottomSheetAnimationSpec`): a 300 ms tween, fast out slow in.
#[allow(non_snake_case)]
pub(crate) fn BottomSheetAnimationSpec() -> Arc<dyn AnimationSpec<f32>> {
    let spec: TweenSpec<f32> = named_args!(tween(duration_millis = 300, easing = Arc::new(FastOutSlowInEasing)));
    Arc::new(spec)
}

const DragHandleVerticalPadding: Dp = Dp(22.0);
const PredictiveBackMaxScaleXDistance: Dp = Dp(48.0);
const PredictiveBackMaxScaleYDistance: Dp = Dp(24.0);
/// Compose's internal `PredictiveBackChildTransformOrigin`.
pub(crate) const PredictiveBackChildTransformOrigin: TransformOrigin = TransformOrigin::new(0.5, 0.0);
