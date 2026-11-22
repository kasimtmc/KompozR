//! Material's floating toolbars, as Compose's FloatingToolbar.kt (material3 1.5, Expressive): a pill of actions
//! floating over the content, laid out across ([`HorizontalFloatingToolbar`]) or down ([`VerticalFloatingToolbar`]),
//! its leading and trailing actions shown while expanded; or a toolbar beside a floating action button that grows out
//! of it as it expands ([`HorizontalFloatingToolbarWithFab`], [`VerticalFloatingToolbarWithFab`]). A scroll behavior
//! ([`FloatingToolbarDefaults::exit_always_scroll_behavior`]) moves it out of view as the content scrolls forwards
//! and back as it scrolls back; [`FloatingToolbarDefaultsModifiers::floating_toolbar_vertical_nested_scroll`] expands
//! and collapses it as a list scrolls. How each kind is made can be replaced below its `Local…Override` (Compose's
//! experimental component override API).
//!
//! Compose caches each scheme's default toolbar colors in the scheme; KompozR's [`ColorScheme`] has no such caches
//! (see `color_scheme.rs`): the defaults are made where they are asked for (the same values).

use crate::color_scheme::ColorScheme;
use crate::content_color::LocalContentColor;
use crate::floating_action_button::{FloatingActionButton, FloatingActionButtonDefaults};
use crate::interactive_component_size::{MinimumInteractiveLeftAlignmentLine, MinimumInteractiveTopAlignmentLine};
use crate::internal::accessibility_service_state_provider::remember_accessibility_service_state;
use crate::internal::child_parent_semantics::ChildParentSemanticsModifiers;
use crate::internal::strings::{Strings, get_string};
use crate::material_theme::MaterialTheme;
use crate::tokens::{
    ColorSchemeKeyTokens, ElevationTokens, FabBaselineTokens, FabMediumTokens, FloatingToolbarTokens,
    MotionSchemeKeyTokens,
};
use kompoz_animation::{
    Animatable, AnimationScope, AnimationSpec, AnimationVector1D, ColumnAnimatedVisibility, DecayAnimationSpec,
    EnterTransition, ExitTransition, FiniteAnimationSpec, RowAnimatedVisibility, Segment, VectorConverter,
    animate_dp_as_state, expand_horizontally, expand_vertically, float_animation_state, remember_spline_based_decay,
    shrink_horizontally, shrink_vertically, update_transition,
};
use kompoz_foundation::gestures::{DraggableModifiers, DraggableState, OnDragStopped, Orientation};
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, IntSize, LayoutDirection, Offset, Shape, lerp_dp};
use kompoz_runtime::kt::KtFloat;
use kompoz_runtime::{
    KompozitionLocal, KompozitionLocalProvider, KotlinProperties, MutableState, Saver, State, derived_state_of,
    kompozable, kompozable_block, list_saver, mutable_float_state_of, mutable_state_of, named, named_args, remember,
    remember_keyed, remember_saveable, remember_saveable_with, remember_updated_state,
};
use kompoz_ui::input::{FlingFuture, NestedScrollConnection, NestedScrollNode, NestedScrollSource};
use kompoz_ui::layout::{
    AlignmentLine, GraphicsLayerScope, Layout, LayoutCoordinates, LayoutModifierNode, Measurable, MeasureResult,
    MeasureScope,
};
use kompoz_ui::modifier::DelegatedNode;
use kompoz_ui::semantics::CustomAccessibilityAction;
use kompoz_ui::unit::Velocity;
use kompoz_ui::{
    Alignment, Constraints, HorizontalAlignment, Modifier, ModifierNode, ModifierNodeElement, NodeCx, NodeHandle,
    VerticalAlignment, node_capabilities,
};
use std::cell::{Cell, RefCell};
use std::fmt;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::sync::Arc;

// ---- HorizontalFloatingToolbar -----------------------------------------------------------------------------------

/// A horizontal floating toolbar (Compose's Expressive `HorizontalFloatingToolbar(expanded, …)`): `content` in a
/// row at least [`FloatingToolbarDefaults::ContainerSize`] tall, centered, `content_padding` in, on `shape` filled
/// with `colors`' toolbar container color (its content in the toolbar content color), casting a shadow of
/// `expanded_shadow_elevation` or `collapsed_shadow_elevation`; `leading_content` and `trailing_content` grow out at
/// its start and end while `expanded` (the content padded to balance its room while they are away). With
/// `scroll_behavior` it moves as that behavior says (it can't take the keyboard's focus while moved out). A screen
/// reader is given an action expanding or collapsing it (and while touch exploration is on it stays expanded and
/// doesn't move). Made by [`LocalHorizontalFloatingToolbarOverride`]'s override
/// ([`DefaultHorizontalFloatingToolbarOverride`] unless provided).
///
/// Kotlin: `HorizontalFloatingToolbar(expanded: Boolean, modifier: Modifier = Modifier, colors: FloatingToolbarColors = FloatingToolbarDefaults.standardFloatingToolbarColors(), contentPadding: PaddingValues = FloatingToolbarDefaults.ContentPadding, scrollBehavior: FloatingToolbarScrollBehavior? = null, shape: Shape = FloatingToolbarDefaults.ContainerShape, leadingContent: @Composable (RowScope.() -> Unit)? = null, trailingContent: @Composable (RowScope.() -> Unit)? = null, expandedShadowElevation: Dp = FloatingToolbarDefaults.ContainerExpandedElevation, collapsedShadowElevation: Dp = FloatingToolbarDefaults.ContainerCollapsedElevation, content: @Composable RowScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn HorizontalFloatingToolbar(
    expanded: bool,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingToolbarDefaults.standard_floating_toolbar_colors())] colors: FloatingToolbarColors,
    #[default(FloatingToolbarDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    #[default(FloatingToolbarDefaults::ContainerShape())] shape: Arc<dyn Shape>,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(FloatingToolbarDefaults::ContainerExpandedElevation)] expanded_shadow_elevation: Dp,
    #[default(FloatingToolbarDefaults::ContainerCollapsedElevation)] collapsed_shadow_elevation: Dp,
    content: impl Fn() + 'static,
) {
    let scope = HorizontalFloatingToolbarOverrideScope {
        is_expanded: expanded,
        modifier,
        colors,
        content_padding,
        scroll_behavior,
        shape,
        leading_content,
        trailing_content,
        expanded_shadow_elevation,
        collapsed_shadow_elevation,
        content: Rc::new(content),
    };
    LocalHorizontalFloatingToolbarOverride.current().HorizontalFloatingToolbar(&scope);
}

/// The default way a horizontal floating toolbar is made (Compose's experimental
/// `DefaultHorizontalFloatingToolbarOverride`).
#[derive(Clone, Copy, Debug)]
pub struct DefaultHorizontalFloatingToolbarOverride;

impl HorizontalFloatingToolbarOverride for DefaultHorizontalFloatingToolbarOverride {
    #[track_caller]
    fn HorizontalFloatingToolbar(&self, scope: &HorizontalFloatingToolbarOverrideScope) {
        let s = scope.clone();
        kompozable_block!({
            let touch_exploration_service_enabled = remember_touch_exploration_service().value();
            let force_collapse: MutableState<bool> = remember_saveable(|| mutable_state_of(false));
            let should_focus = should_focus_state(s.scroll_behavior.clone());
            let on_a11y_force_collapse: Rc<dyn Fn(bool)> = {
                let force_collapse = force_collapse.clone();
                Rc::new(move |force| force_collapse.set(force))
            };
            HorizontalFloatingToolbarLayout(
                // Make sure that an offscreen toolbar is not keyboard focusable.
                s.modifier.clone().then(unfocusable_unless(should_focus.value())),
                !force_collapse.value() && (touch_exploration_service_enabled || s.is_expanded),
                on_a11y_force_collapse,
                s.colors,
                s.content_padding.clone(),
                if !touch_exploration_service_enabled { s.scroll_behavior.clone() } else { None },
                s.shape.clone(),
                s.leading_content.clone(),
                s.trailing_content.clone(),
                s.expanded_shadow_elevation,
                s.collapsed_shadow_elevation,
                s.content.clone(),
            );
        })
    }
}

/// Compose's `remember { derivedStateOf { (scrollBehavior?.state?.offset ?: 0f) == 0f } }`: whether the toolbar is
/// where it rests (the scroll behavior of the first composition, as Compose's `remember` without keys keeps).
#[kompozable]
fn should_focus_state(scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>) -> State<bool> {
    remember(move || {
        let scroll_behavior = scroll_behavior.clone();
        State::from(derived_state_of(move || scroll_behavior.as_ref().map_or(0.0, |b| b.state().offset()) == 0.0))
    })
}

/// Nothing while `focusable`, else a modifier keeping the keyboard's focus off.
fn unfocusable_unless(focusable: bool) -> Modifier {
    if focusable { Modifier } else { Modifier.focus_properties(|p| p.can_focus = false) }
}

/// A horizontal floating toolbar beside a floating action button (Compose's Expressive
/// `HorizontalFloatingToolbar(expanded, floatingActionButton, …)`): `floating_action_button` at
/// `floating_action_button_position` (end or start), [`FloatingToolbarDefaults::FabSizeRange`]'s largest size (80
/// dp) collapsed, shrinking to its smallest (56 dp) as `content` — a scrolling row `content_padding` in, on `shape`
/// filled with `colors`' toolbar container color, casting a shadow from `collapsed_shadow_elevation` to
/// `expanded_shadow_elevation` — grows out beside it, 8 dp from it, as `expanded` turns on (along
/// `animation_spec`). With `scroll_behavior` both move as it says. A screen reader is given an action expanding or
/// collapsing it. Made by [`LocalHorizontalFloatingToolbarWithFabOverride`]'s override
/// ([`DefaultHorizontalFloatingToolbarWithFabOverride`] unless provided).
///
/// Kotlin: `HorizontalFloatingToolbar(expanded: Boolean, floatingActionButton: @Composable () -> Unit, modifier: Modifier = Modifier, colors: FloatingToolbarColors = FloatingToolbarDefaults.standardFloatingToolbarColors(), contentPadding: PaddingValues = FloatingToolbarDefaults.ContentPadding, scrollBehavior: FloatingToolbarScrollBehavior? = null, shape: Shape = FloatingToolbarDefaults.ContainerShape, floatingActionButtonPosition: FloatingToolbarHorizontalFabPosition = FloatingToolbarHorizontalFabPosition.End, animationSpec: FiniteAnimationSpec<Float> = FloatingToolbarDefaults.animationSpec(), expandedShadowElevation: Dp = FloatingToolbarDefaults.ContainerExpandedElevationWithFab, collapsedShadowElevation: Dp = FloatingToolbarDefaults.ContainerCollapsedElevationWithFab, content: @Composable RowScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn HorizontalFloatingToolbarWithFab(
    expanded: bool,
    floating_action_button: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingToolbarDefaults.standard_floating_toolbar_colors())] colors: FloatingToolbarColors,
    #[default(FloatingToolbarDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    #[default(FloatingToolbarDefaults::ContainerShape())] shape: Arc<dyn Shape>,
    #[default(FloatingToolbarHorizontalFabPosition::End)]
    floating_action_button_position: FloatingToolbarHorizontalFabPosition,
    #[default(FloatingToolbarDefaults.animation_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    #[default(FloatingToolbarDefaults::ContainerExpandedElevationWithFab)] expanded_shadow_elevation: Dp,
    #[default(FloatingToolbarDefaults::ContainerCollapsedElevationWithFab)] collapsed_shadow_elevation: Dp,
    content: impl Fn() + 'static,
) {
    let scope = HorizontalFloatingToolbarWithFabOverrideScope {
        is_expanded: expanded,
        floating_action_button: Rc::new(floating_action_button),
        modifier,
        colors,
        content_padding,
        scroll_behavior,
        shape,
        floating_action_button_position,
        animation_spec,
        expanded_shadow_elevation,
        collapsed_shadow_elevation,
        content: Rc::new(content),
    };
    LocalHorizontalFloatingToolbarWithFabOverride.current().HorizontalFloatingToolbarWithFab(&scope);
}

/// The default way a horizontal floating toolbar with a FAB is made (Compose's experimental
/// `DefaultHorizontalFloatingToolbarWithFabOverride`).
#[derive(Clone, Copy, Debug)]
pub struct DefaultHorizontalFloatingToolbarWithFabOverride;

impl HorizontalFloatingToolbarWithFabOverride for DefaultHorizontalFloatingToolbarWithFabOverride {
    #[track_caller]
    fn HorizontalFloatingToolbarWithFab(&self, scope: &HorizontalFloatingToolbarWithFabOverrideScope) {
        let s = scope.clone();
        kompozable_block!({
            let touch_exploration_service_enabled = remember_touch_exploration_service().value();
            let force_collapse: MutableState<bool> = remember_saveable(|| mutable_state_of(false));
            let on_a11y_force_collapse: Rc<dyn Fn(bool)> = {
                let force_collapse = force_collapse.clone();
                Rc::new(move |force| force_collapse.set(force))
            };
            HorizontalFloatingToolbarWithFabLayout(
                s.modifier.clone(),
                !force_collapse.value() && (touch_exploration_service_enabled || s.is_expanded),
                on_a11y_force_collapse,
                s.colors,
                FloatingToolbarDefaults::ToolbarToFabGap,
                s.content_padding.clone(),
                if !touch_exploration_service_enabled { s.scroll_behavior.clone() } else { None },
                s.shape.clone(),
                s.animation_spec.clone(),
                s.floating_action_button.clone(),
                s.floating_action_button_position,
                s.expanded_shadow_elevation,
                s.collapsed_shadow_elevation,
                s.content.clone(),
            );
        })
    }
}

// ---- VerticalFloatingToolbar -------------------------------------------------------------------------------------

/// A vertical floating toolbar (Compose's Expressive `VerticalFloatingToolbar(expanded, …)`): `content` in a column
/// at least [`FloatingToolbarDefaults::ContainerSize`] wide, centered, `content_padding` in, on `shape` filled with
/// `colors`' toolbar container color, casting a shadow of `expanded_shadow_elevation` or
/// `collapsed_shadow_elevation`; `leading_content` and `trailing_content` grow out above and below while `expanded`.
/// With `scroll_behavior` it moves as that behavior says. A screen reader is given an action expanding or
/// collapsing it. Made by [`LocalVerticalToolbarOverride`]'s override ([`DefaultVerticalFloatingToolbarOverride`]
/// unless provided).
///
/// Kotlin: `VerticalFloatingToolbar(expanded: Boolean, modifier: Modifier = Modifier, colors: FloatingToolbarColors = FloatingToolbarDefaults.standardFloatingToolbarColors(), contentPadding: PaddingValues = FloatingToolbarDefaults.ContentPadding, scrollBehavior: FloatingToolbarScrollBehavior? = null, shape: Shape = FloatingToolbarDefaults.ContainerShape, leadingContent: @Composable (ColumnScope.() -> Unit)? = null, trailingContent: @Composable (ColumnScope.() -> Unit)? = null, expandedShadowElevation: Dp = FloatingToolbarDefaults.ContainerExpandedElevation, collapsedShadowElevation: Dp = FloatingToolbarDefaults.ContainerCollapsedElevation, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn VerticalFloatingToolbar(
    expanded: bool,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingToolbarDefaults.standard_floating_toolbar_colors())] colors: FloatingToolbarColors,
    #[default(FloatingToolbarDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    #[default(FloatingToolbarDefaults::ContainerShape())] shape: Arc<dyn Shape>,
    #[default(None)] leading_content: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_content: Option<Rc<dyn Fn()>>,
    #[default(FloatingToolbarDefaults::ContainerExpandedElevation)] expanded_shadow_elevation: Dp,
    #[default(FloatingToolbarDefaults::ContainerCollapsedElevation)] collapsed_shadow_elevation: Dp,
    content: impl Fn() + 'static,
) {
    let scope = VerticalFloatingToolbarOverrideScope {
        is_expanded: expanded,
        modifier,
        colors,
        content_padding,
        scroll_behavior,
        shape,
        leading_content,
        trailing_content,
        expanded_shadow_elevation,
        collapsed_shadow_elevation,
        content: Rc::new(content),
    };
    LocalVerticalToolbarOverride.current().VerticalFloatingToolbar(&scope);
}

/// The default way a vertical floating toolbar is made (Compose's experimental
/// `DefaultVerticalFloatingToolbarOverride`).
#[derive(Clone, Copy, Debug)]
pub struct DefaultVerticalFloatingToolbarOverride;

impl VerticalFloatingToolbarOverride for DefaultVerticalFloatingToolbarOverride {
    #[track_caller]
    fn VerticalFloatingToolbar(&self, scope: &VerticalFloatingToolbarOverrideScope) {
        let s = scope.clone();
        kompozable_block!({
            let touch_exploration_service_enabled = remember_touch_exploration_service().value();
            let force_collapse: MutableState<bool> = remember_saveable(|| mutable_state_of(false));
            let should_focus = should_focus_state(s.scroll_behavior.clone());
            let on_a11y_force_collapse: Rc<dyn Fn(bool)> = {
                let force_collapse = force_collapse.clone();
                Rc::new(move |force| force_collapse.set(force))
            };
            VerticalFloatingToolbarLayout(
                // Make sure that an offscreen toolbar is not keyboard focusable.
                s.modifier.clone().then(unfocusable_unless(should_focus.value())),
                !force_collapse.value() && (touch_exploration_service_enabled || s.is_expanded),
                on_a11y_force_collapse,
                s.colors,
                s.content_padding.clone(),
                if !touch_exploration_service_enabled { s.scroll_behavior.clone() } else { None },
                s.shape.clone(),
                s.leading_content.clone(),
                s.trailing_content.clone(),
                s.expanded_shadow_elevation,
                s.collapsed_shadow_elevation,
                s.content.clone(),
            );
        })
    }
}

/// A vertical floating toolbar beside a floating action button (Compose's Expressive
/// `VerticalFloatingToolbar(expanded, floatingActionButton, …)`): `floating_action_button` at
/// `floating_action_button_position` (bottom or top), 80 dp collapsed, shrinking to 56 dp as `content` — a
/// scrolling column `content_padding` in, on `shape` filled with `colors`' toolbar container color — grows out
/// above (or below) it, 8 dp from it, as `expanded` turns on (along `animation_spec`). With `scroll_behavior` both
/// move as it says. Made by [`LocalVerticalToolbarWithFabOverride`]'s override
/// ([`DefaultVerticalFloatingToolbarWithFabOverride`] unless provided).
///
/// Kotlin: `VerticalFloatingToolbar(expanded: Boolean, floatingActionButton: @Composable () -> Unit, modifier: Modifier = Modifier, colors: FloatingToolbarColors = FloatingToolbarDefaults.standardFloatingToolbarColors(), contentPadding: PaddingValues = FloatingToolbarDefaults.ContentPadding, scrollBehavior: FloatingToolbarScrollBehavior? = null, shape: Shape = FloatingToolbarDefaults.ContainerShape, floatingActionButtonPosition: FloatingToolbarVerticalFabPosition = FloatingToolbarVerticalFabPosition.Bottom, animationSpec: FiniteAnimationSpec<Float> = FloatingToolbarDefaults.animationSpec(), expandedShadowElevation: Dp = FloatingToolbarDefaults.ContainerExpandedElevationWithFab, collapsedShadowElevation: Dp = FloatingToolbarDefaults.ContainerCollapsedElevationWithFab, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn VerticalFloatingToolbarWithFab(
    expanded: bool,
    floating_action_button: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(FloatingToolbarDefaults.standard_floating_toolbar_colors())] colors: FloatingToolbarColors,
    #[default(FloatingToolbarDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(None)] scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    #[default(FloatingToolbarDefaults::ContainerShape())] shape: Arc<dyn Shape>,
    #[default(FloatingToolbarVerticalFabPosition::Bottom)]
    floating_action_button_position: FloatingToolbarVerticalFabPosition,
    #[default(FloatingToolbarDefaults.animation_spec())] animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    #[default(FloatingToolbarDefaults::ContainerExpandedElevationWithFab)] expanded_shadow_elevation: Dp,
    #[default(FloatingToolbarDefaults::ContainerCollapsedElevationWithFab)] collapsed_shadow_elevation: Dp,
    content: impl Fn() + 'static,
) {
    let scope = VerticalFloatingToolbarWithFabOverrideScope {
        is_expanded: expanded,
        floating_action_button: Rc::new(floating_action_button),
        modifier,
        colors,
        content_padding,
        scroll_behavior,
        shape,
        floating_action_button_position,
        animation_spec,
        expanded_shadow_elevation,
        collapsed_shadow_elevation,
        content: Rc::new(content),
    };
    LocalVerticalToolbarWithFabOverride.current().VerticalFloatingToolbarWithFab(&scope);
}

/// The default way a vertical floating toolbar with a FAB is made (Compose's experimental
/// `DefaultVerticalFloatingToolbarWithFabOverride`).
#[derive(Clone, Copy, Debug)]
pub struct DefaultVerticalFloatingToolbarWithFabOverride;

impl VerticalFloatingToolbarWithFabOverride for DefaultVerticalFloatingToolbarWithFabOverride {
    #[track_caller]
    fn VerticalFloatingToolbarWithFab(&self, scope: &VerticalFloatingToolbarWithFabOverrideScope) {
        let s = scope.clone();
        kompozable_block!({
            let touch_exploration_service_enabled = remember_touch_exploration_service().value();
            let force_collapse: MutableState<bool> = remember_saveable(|| mutable_state_of(false));
            let on_a11y_force_collapse: Rc<dyn Fn(bool)> = {
                let force_collapse = force_collapse.clone();
                Rc::new(move |force| force_collapse.set(force))
            };
            VerticalFloatingToolbarWithFabLayout(
                s.modifier.clone(),
                !force_collapse.value() && (touch_exploration_service_enabled || s.is_expanded),
                on_a11y_force_collapse,
                s.colors,
                FloatingToolbarDefaults::ToolbarToFabGap,
                s.content_padding.clone(),
                if !touch_exploration_service_enabled { s.scroll_behavior.clone() } else { None },
                s.shape.clone(),
                s.animation_spec.clone(),
                s.floating_action_button.clone(),
                s.floating_action_button_position,
                s.expanded_shadow_elevation,
                s.collapsed_shadow_elevation,
                s.content.clone(),
            );
        })
    }
}

// ---- FloatingToolbarScrollBehavior -------------------------------------------------------------------------------

mod sealed {
    /// Seals [`FloatingToolbarScrollBehavior`](super::FloatingToolbarScrollBehavior) (Compose's `sealed
    /// interface`): only this crate's behaviors are ones.
    pub trait Sealed {}
}

/// How a floating toolbar moves as the content scrolls (Compose's Expressive sealed interface
/// `FloatingToolbarScrollBehavior`, a nested scroll connection): give it to the toolbar's `scroll_behavior` and, as
/// a [`NestedScrollConnection`], to a `Modifier.nested_scroll` above the content. Shared as `Rc<dyn
/// FloatingToolbarScrollBehavior>`; [`ExitAlwaysFloatingToolbarScrollBehavior`] is the one there is.
pub trait FloatingToolbarScrollBehavior: NestedScrollConnection + sealed::Sealed {
    /// The way the toolbar leaves (Compose's `exitDirection`).
    fn exit_direction(&self) -> FloatingToolbarExitDirection;

    /// Where the toolbar is (Compose's `state`).
    fn state(&self) -> Rc<dyn FloatingToolbarState>;

    /// What settles it after a drag or a fling, at rest or out of view (Compose's `snapAnimationSpec`).
    fn snap_animation_spec(&self) -> Arc<dyn AnimationSpec<f32>>;

    /// What carries on a fling's velocity (Compose's `flingAnimationSpec`).
    fn fling_animation_spec(&self) -> Arc<dyn DecayAnimationSpec<f32>>;

    /// `modifier` then what moves the toolbar by its state's offset, lets it be dragged and tells its state how far
    /// it can go (Compose's member extension `Modifier.floatingScrollBehavior()`; the toolbar applies it).
    fn floating_scroll_behavior(&self, modifier: Modifier) -> Modifier;
}

/// A scroll behavior moving the toolbar out of view as the content scrolls forwards, back as it scrolls back, also
/// dragged by the user, settling at rest or out of view when let go (Compose's Expressive
/// `ExitAlwaysFloatingToolbarScrollBehavior`; made by [`FloatingToolbarDefaults::exit_always_scroll_behavior`]).
#[derive(Clone)]
pub struct ExitAlwaysFloatingToolbarScrollBehavior {
    exit_direction: FloatingToolbarExitDirection,
    state: Rc<dyn FloatingToolbarState>,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    fling_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
}

impl ExitAlwaysFloatingToolbarScrollBehavior {
    /// Kotlin: `ExitAlwaysFloatingToolbarScrollBehavior(exitDirection: FloatingToolbarExitDirection, state: FloatingToolbarState, snapAnimationSpec: AnimationSpec<Float>, flingAnimationSpec: DecayAnimationSpec<Float>)`
    pub fn new(
        exit_direction: FloatingToolbarExitDirection,
        state: Rc<dyn FloatingToolbarState>,
        snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
        fling_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    ) -> ExitAlwaysFloatingToolbarScrollBehavior {
        ExitAlwaysFloatingToolbarScrollBehavior { exit_direction, state, snap_animation_spec, fling_animation_spec }
    }
}

impl sealed::Sealed for ExitAlwaysFloatingToolbarScrollBehavior {}

impl NestedScrollConnection for ExitAlwaysFloatingToolbarScrollBehavior {
    fn on_post_scroll(&self, consumed: Offset, _available: Offset, _source: NestedScrollSource) -> Offset {
        let state = &self.state;
        state.set_content_offset(state.content_offset() + consumed.y);
        state.set_offset(state.offset() + consumed.y);
        Offset::Zero
    }

    fn on_post_fling(&self, consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        let (state, snap, fling) =
            (self.state.clone(), self.snap_animation_spec.clone(), self.fling_animation_spec.clone());
        Box::pin(async move {
            if available.y > 0.0 && (state.offset() == 0.0 || state.offset() == state.offset_limit()) {
                // Reset the total content offset to zero when scrolling all the way down.
                // This will eliminate some float precision inaccuracies.
                state.set_content_offset(0.0);
            }
            // (`super.onPostFling`: NestedScrollConnection's, nothing)
            let _ = consumed;
            let super_consumed = Velocity::Zero;
            super_consumed + settle_floating_toolbar(state, available.y, snap, fling).await
        })
    }
}

impl FloatingToolbarScrollBehavior for ExitAlwaysFloatingToolbarScrollBehavior {
    fn exit_direction(&self) -> FloatingToolbarExitDirection {
        self.exit_direction
    }

    fn state(&self) -> Rc<dyn FloatingToolbarState> {
        self.state.clone()
    }

    fn snap_animation_spec(&self) -> Arc<dyn AnimationSpec<f32>> {
        self.snap_animation_spec.clone()
    }

    fn fling_animation_spec(&self) -> Arc<dyn DecayAnimationSpec<f32>> {
        self.fling_animation_spec.clone()
    }

    fn floating_scroll_behavior(&self, modifier: Modifier) -> Modifier {
        use FloatingToolbarExitDirection as D;
        let exit_direction = self.exit_direction;
        let horizontal = exit_direction == D::Start || exit_direction == D::End;
        let is_rtl = Rc::new(Cell::new(false));
        let orientation = if horizontal { Orientation::Horizontal } else { Orientation::Vertical };
        let draggable_state = {
            let (state, is_rtl) = (self.state.clone(), is_rtl.clone());
            DraggableState(move |delta| {
                let offset = if horizontal && is_rtl.get() { -delta } else { delta };
                if exit_direction == D::Start || exit_direction == D::Top {
                    state.set_offset(state.offset() + offset);
                } else {
                    state.set_offset(state.offset() - offset);
                }
            })
        };
        let on_drag_stopped: OnDragStopped = {
            let (state, snap, fling) =
                (self.state.clone(), self.snap_animation_spec.clone(), self.fling_animation_spec.clone());
            Rc::new(move |_, velocity| {
                let (state, snap, fling) = (state.clone(), snap.clone(), fling.clone());
                Box::pin(async move {
                    settle_floating_toolbar(state, velocity, snap, fling).await;
                })
            })
        };
        let (layout_state, layout_rtl) = (self.state.clone(), is_rtl.clone());
        let (positioned_state, positioned_rtl) = (self.state.clone(), is_rtl);
        let moved = modifier.layout(move |scope, measurable, constraints| {
            layout_rtl.set(scope.layout_direction() == LayoutDirection::Rtl);
            // Sets the toolbar's offset to collapse the entire bar's when content scrolled.
            let placeable = measurable.measure(constraints);
            let offset = if horizontal && layout_rtl.get() { -layout_state.offset() } else { layout_state.offset() };
            let (width, height) = (placeable.width(), placeable.height());
            named_args!(scope.layout(width, height, move |p| {
                let at = offset.round_to_int();
                match exit_direction {
                    D::Start => p.place_with_layer(&placeable, at, 0, 0.0, |_: &mut GraphicsLayerScope| {}),
                    D::End => p.place_with_layer(&placeable, -at, 0, 0.0, |_: &mut GraphicsLayerScope| {}),
                    D::Top => p.place_with_layer(&placeable, 0, at, 0.0, |_: &mut GraphicsLayerScope| {}),
                    _ => p.place_with_layer(&placeable, 0, -at, 0.0, |_: &mut GraphicsLayerScope| {}),
                }
            }))
        });
        let dragged =
            named_args!(moved.draggable(draggable_state, orientation, on_drag_stopped = Some(on_drag_stopped)));
        dragged.on_globally_positioned(move |coordinates: &LayoutCoordinates| {
            // Updates the toolbar's offsetLimit relative to the parent.
            let parent_offset = coordinates.position_in_parent();
            let parent_size = coordinates.parent_layout_coordinates().map_or(IntSize::Zero, |c| c.size());
            let (width, height) = (coordinates.size().width as f32, coordinates.size().height as f32);
            let (parent_width, parent_height) = (parent_size.width as f32, parent_size.height as f32);
            let is_rtl = positioned_rtl.get();
            let limit = match exit_direction {
                D::Start => {
                    if is_rtl {
                        parent_width - parent_offset.x
                    } else {
                        width + parent_offset.x
                    }
                }
                D::End => {
                    if is_rtl {
                        width + parent_offset.x
                    } else {
                        parent_width - parent_offset.x
                    }
                }
                D::Top => height + parent_offset.y,
                _ => parent_height - parent_offset.y,
            };
            positioned_state.set_offset_limit(-(limit - positioned_state.offset()));
        })
    }
}

// ---- FloatingToolbarDefaults -------------------------------------------------------------------------------------

/// The floating toolbars' defaults (Compose's Expressive `FloatingToolbarDefaults`): their sizes, elevations, shape,
/// padding, motion, scroll behavior, transitions, colors and FABs. Plain values are associated consts; what Compose
/// reads in composition is an associated function, what takes arguments a method of the value
/// (`FloatingToolbarDefaults.standard_floating_toolbar_colors()`).
#[derive(Clone, Copy, Debug)]
pub struct FloatingToolbarDefaults;

impl FloatingToolbarDefaults {
    /// A toolbar's least height (across) or width (down), 64 dp (Compose's `ContainerSize`).
    pub const ContainerSize: Dp = FloatingToolbarTokens::ContainerHeight;

    /// An expanded toolbar's shadow, none (Compose's `ContainerExpandedElevation`).
    pub const ContainerExpandedElevation: Dp = ElevationTokens::Level0; // TODO read from token

    /// A collapsed toolbar's shadow, none (Compose's `ContainerCollapsedElevation`).
    pub const ContainerCollapsedElevation: Dp = ElevationTokens::Level0; // TODO read from token

    /// An expanded toolbar's shadow beside a FAB, 1 dp (Compose's `ContainerExpandedElevationWithFab`).
    pub const ContainerExpandedElevationWithFab: Dp = ElevationTokens::Level1; // TODO read from token

    /// A collapsed toolbar's shadow beside a FAB, none (Compose's `ContainerCollapsedElevationWithFab`).
    pub const ContainerCollapsedElevationWithFab: Dp = ElevationTokens::Level0; // TODO read from token

    /// A toolbar's shape, the theme's full corners: a pill (Compose's `ContainerShape`).
    pub fn ContainerShape() -> Arc<dyn Shape> {
        FloatingToolbarTokens::ContainerShape.value()
    }

    /// The space around a toolbar's content, 8 dp at each side (Compose's `ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::new(
            FloatingToolbarTokens::ContainerLeadingSpace,
            FloatingToolbarTokens::ContainerLeadingSpace,
            FloatingToolbarTokens::ContainerTrailingSpace,
            FloatingToolbarTokens::ContainerTrailingSpace,
        )
    }

    /// How far from the screen's edges a toolbar should float, 16 dp (Compose's `ScreenOffset`).
    pub const ScreenOffset: Dp = FloatingToolbarTokens::ContainerExternalPadding;

    /// How far the content scrolls before [`floating_toolbar_vertical_nested_scroll`](FloatingToolbarDefaultsModifiers::floating_toolbar_vertical_nested_scroll)
    /// expands or collapses a toolbar, 40 dp (Compose's `ScrollDistanceThreshold`).
    pub const ScrollDistanceThreshold: Dp = Dp(40.0);

    /// The sizes a toolbar's FAB takes, from expanded (56 dp) to collapsed (80 dp) (Compose's internal
    /// `FabSizeRange`).
    pub(crate) const FabSizeRange: RangeInclusive<Dp> =
        FabBaselineTokens::ContainerWidth..=FabMediumTokens::ContainerWidth;

    // TODO Load this from the component tokens?
    /// The space between a toolbar and its FAB, 8 dp (Compose's internal `ToolbarToFabGap`).
    pub(crate) const ToolbarToFabGap: Dp = Dp(8.0);

    /// The standard colors of `scheme` (Compose's internal `ColorScheme.defaultFloatingToolbarStandardColors`,
    /// made each time: no cache in the scheme): the surface container's, the primary container's for the FAB.
    pub(crate) fn default_floating_toolbar_standard_colors(scheme: &ColorScheme) -> FloatingToolbarColors {
        // TODO load colors from the toolbar tokens. If possible, remove the usage of contentColorFor.
        let container = scheme.from_token(FloatingToolbarTokens::StandardContainerColor);
        let fab_container = scheme.from_token(ColorSchemeKeyTokens::PrimaryContainer);
        FloatingToolbarColors::new(
            container,
            scheme.content_color_for(container),
            fab_container,
            scheme.content_color_for(fab_container),
        )
    }

    /// The vibrant colors of `scheme` (Compose's internal `ColorScheme.defaultFloatingToolbarVibrantColors`): the
    /// primary container's, the tertiary container's for the FAB.
    pub(crate) fn default_floating_toolbar_vibrant_colors(scheme: &ColorScheme) -> FloatingToolbarColors {
        // TODO load colors from the toolbar tokens. If possible, remove the usage of contentColorFor.
        let container = scheme.from_token(FloatingToolbarTokens::VibrantContainerColor);
        let fab_container = scheme.from_token(ColorSchemeKeyTokens::TertiaryContainer);
        FloatingToolbarColors::new(
            container,
            scheme.content_color_for(container),
            fab_container,
            scheme.content_color_for(fab_container),
        )
    }
}

#[named]
impl FloatingToolbarDefaults {
    /// How a toolbar moves as it expands and collapses: the theme's fast spatial motion (Compose's
    /// `animationSpec<T>()`).
    pub fn animation_spec<T: 'static>(&self) -> Arc<dyn FiniteAnimationSpec<T>> {
        // TODO Load the motionScheme tokens from the component tokens file
        MotionSchemeKeyTokens::FastSpatial.value::<T>()
    }

    /// A scroll behavior moving a toolbar out of view towards `exit_direction` as the content scrolls forwards,
    /// back as it scrolls back ([`ExitAlwaysFloatingToolbarScrollBehavior`], remembered while its arguments stay;
    /// Compose's `exitAlwaysScrollBehavior`): `state` its state (None: a remembered
    /// [`remember_floating_toolbar_state`]), settling along `snap_animation_spec`, a fling carried on by
    /// `fling_animation_spec` (None: the platform's fling, `rememberSplineBasedDecay()`). Note that it may make the
    /// toolbar hard to reach with assistive technologies.
    ///
    /// Kotlin: `exitAlwaysScrollBehavior(exitDirection: FloatingToolbarExitDirection, state: FloatingToolbarState = rememberFloatingToolbarState(), snapAnimationSpec: AnimationSpec<Float> = MotionSchemeKeyTokens.DefaultEffects.value(), flingAnimationSpec: DecayAnimationSpec<Float> = rememberSplineBasedDecay())`
    #[track_caller]
    pub fn exit_always_scroll_behavior(
        &self,
        exit_direction: FloatingToolbarExitDirection,
        #[default(None)] state: Option<Rc<dyn FloatingToolbarState>>,
        #[default(default_effects_spec())] snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
        #[default(None)] fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    ) -> Rc<dyn FloatingToolbarScrollBehavior> {
        kompozable_block!({
            let state = match state {
                Some(state) => state,
                None => remember_floating_toolbar_state(),
            };
            let fling_animation_spec = match fling_animation_spec {
                Some(spec) => spec,
                None => remembered_spline_based_decay(),
            };
            // (Compose's `remember(…)` keys: the decay by its identity, as Kotlin's equals of a decay spec)
            let fling_key = Arc::as_ptr(&fling_animation_spec) as *const () as usize;
            remember_keyed((exit_direction, state.clone(), snap_animation_spec.clone(), fling_key), move || {
                Rc::new(ExitAlwaysFloatingToolbarScrollBehavior::new(
                    exit_direction,
                    state,
                    snap_animation_spec,
                    fling_animation_spec,
                )) as Rc<dyn FloatingToolbarScrollBehavior>
            })
        })
    }

    /// How a toolbar's side content comes in across, growing from `expand_from` (Compose's
    /// `horizontalEnterTransition`).
    pub fn horizontal_enter_transition(&self, expand_from: HorizontalAlignment) -> EnterTransition {
        named_args!(expand_horizontally(animation_spec = self.animation_spec(), expand_from = expand_from))
    }

    /// How a toolbar's side content comes in down or up, growing from `expand_from` (Compose's
    /// `verticalEnterTransition`).
    pub fn vertical_enter_transition(&self, expand_from: VerticalAlignment) -> EnterTransition {
        named_args!(expand_vertically(animation_spec = self.animation_spec(), expand_from = expand_from))
    }

    /// How a toolbar's side content leaves across, shrinking towards `shrink_towards` (Compose's
    /// `horizontalExitTransition`).
    pub fn horizontal_exit_transition(&self, shrink_towards: HorizontalAlignment) -> ExitTransition {
        named_args!(shrink_horizontally(animation_spec = self.animation_spec(), shrink_towards = shrink_towards))
    }

    /// How a toolbar's side content leaves down or up, shrinking towards `shrink_towards` (Compose's
    /// `verticalExitTransition`).
    pub fn vertical_exit_transition(&self, shrink_towards: VerticalAlignment) -> ExitTransition {
        named_args!(shrink_vertically(animation_spec = self.animation_spec(), shrink_towards = shrink_towards))
    }

    /// A standard toolbar's colors, each given one other than unspecified in place of the theme's (Compose's
    /// `standardFloatingToolbarColors()` and `standardFloatingToolbarColors(toolbarContainerColor, …)`): the surface
    /// container with its content color, the primary container for the FAB.
    pub fn standard_floating_toolbar_colors(
        &self,
        #[default(Color::Unspecified)] toolbar_container_color: Color,
        #[default(Color::Unspecified)] toolbar_content_color: Color,
        #[default(Color::Unspecified)] fab_container_color: Color,
        #[default(Color::Unspecified)] fab_content_color: Color,
    ) -> FloatingToolbarColors {
        named_args!(Self::default_floating_toolbar_standard_colors(&MaterialTheme::color_scheme()).copy(
            toolbar_container_color = toolbar_container_color,
            toolbar_content_color = toolbar_content_color,
            fab_container_color = fab_container_color,
            fab_content_color = fab_content_color,
        ))
    }

    /// A vibrant toolbar's colors, each given one other than unspecified in place of the theme's (Compose's
    /// `vibrantFloatingToolbarColors()` and `vibrantFloatingToolbarColors(toolbarContainerColor, …)`): the primary
    /// container with its content color, the tertiary container for the FAB.
    pub fn vibrant_floating_toolbar_colors(
        &self,
        #[default(Color::Unspecified)] toolbar_container_color: Color,
        #[default(Color::Unspecified)] toolbar_content_color: Color,
        #[default(Color::Unspecified)] fab_container_color: Color,
        #[default(Color::Unspecified)] fab_content_color: Color,
    ) -> FloatingToolbarColors {
        named_args!(Self::default_floating_toolbar_vibrant_colors(&MaterialTheme::color_scheme()).copy(
            toolbar_container_color = toolbar_container_color,
            toolbar_content_color = toolbar_content_color,
            fab_container_color = fab_container_color,
            fab_content_color = fab_content_color,
        ))
    }

    /// The FAB of a vibrant toolbar (Compose's `VibrantFloatingActionButton`): a [`FloatingActionButton`] filling
    /// the room the toolbar gives it, in the vibrant colors' FAB colors, 6 dp of shadow (8 hovered).
    ///
    /// Kotlin: `VibrantFloatingActionButton(onClick: () -> Unit, modifier: Modifier = Modifier, shape: Shape = FloatingActionButtonDefaults.shape, containerColor: Color = vibrantFloatingToolbarColors().fabContainerColor, contentColor: Color = vibrantFloatingToolbarColors().fabContentColor, interactionSource: MutableInteractionSource? = null, content: @Composable () -> Unit)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn VibrantFloatingActionButton(
        &self,
        on_click: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(FloatingActionButtonDefaults::shape())] shape: Arc<dyn Shape>,
        #[default(vibrant_colors().fab_container_color)] container_color: Color,
        #[default(vibrant_colors().fab_content_color)] content_color: Color,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            ToolbarFloatingActionButton(
                Rc::new(on_click),
                modifier,
                shape,
                container_color,
                content_color,
                interaction_source,
                Rc::new(content),
            );
        })
    }

    /// The FAB of a standard toolbar (Compose's `StandardFloatingActionButton`): a [`FloatingActionButton`] filling
    /// the room the toolbar gives it, in the standard colors' FAB colors, 6 dp of shadow (8 hovered).
    ///
    /// Kotlin: `StandardFloatingActionButton(onClick: () -> Unit, modifier: Modifier = Modifier, shape: Shape = FloatingActionButtonDefaults.shape, containerColor: Color = standardFloatingToolbarColors().fabContainerColor, contentColor: Color = standardFloatingToolbarColors().fabContentColor, interactionSource: MutableInteractionSource? = null, content: @Composable () -> Unit)`
    #[track_caller]
    #[allow(clippy::too_many_arguments)]
    pub fn StandardFloatingActionButton(
        &self,
        on_click: impl Fn() + 'static,
        #[default(Modifier)] modifier: Modifier,
        #[default(FloatingActionButtonDefaults::shape())] shape: Arc<dyn Shape>,
        #[default(standard_colors().fab_container_color)] container_color: Color,
        #[default(standard_colors().fab_content_color)] content_color: Color,
        #[default(None)] interaction_source: Option<MutableInteractionSource>,
        content: impl Fn() + 'static,
    ) {
        kompozable_block!({
            ToolbarFloatingActionButton(
                Rc::new(on_click),
                modifier,
                shape,
                container_color,
                content_color,
                interaction_source,
                Rc::new(content),
            );
        })
    }
}

/// The theme's default effects motion (`MotionSchemeKeyTokens.DefaultEffects.value()`), as a spec of floats.
fn default_effects_spec() -> Arc<dyn AnimationSpec<f32>> {
    MotionSchemeKeyTokens::DefaultEffects.value::<f32>()
}

/// The theme's standard toolbar colors (`standardFloatingToolbarColors()`).
fn standard_colors() -> FloatingToolbarColors {
    FloatingToolbarDefaults::default_floating_toolbar_standard_colors(&MaterialTheme::color_scheme())
}

/// The theme's vibrant toolbar colors (`vibrantFloatingToolbarColors()`).
fn vibrant_colors() -> FloatingToolbarColors {
    FloatingToolbarDefaults::default_floating_toolbar_vibrant_colors(&MaterialTheme::color_scheme())
}

/// The platform's fling (`rememberSplineBasedDecay()`), one shared spec while the density stays.
#[kompozable]
fn remembered_spline_based_decay() -> Arc<dyn DecayAnimationSpec<f32>> {
    let decay = remember_spline_based_decay::<f32>();
    let key = Arc::as_ptr(decay.float_decay_spec()) as *const () as usize;
    remember_keyed(key, move || Arc::new(decay) as Arc<dyn DecayAnimationSpec<f32>>)
}

/// What both toolbar FABs are (the body of Compose's `VibrantFloatingActionButton` and
/// `StandardFloatingActionButton`).
#[kompozable]
fn ToolbarFloatingActionButton(
    on_click: Rc<dyn Fn()>,
    modifier: Modifier,
    shape: Arc<dyn Shape>,
    container_color: Color,
    content_color: Color,
    interaction_source: Option<MutableInteractionSource>,
    content: Rc<dyn Fn()>,
) {
    FloatingActionButton(
        on_click = move || on_click(),
        modifier = modifier.fill_max_size(),
        shape = shape,
        container_color = container_color,
        content_color = Some(content_color),
        elevation = FloatingActionButtonDefaults.elevation(
            default_elevation = ElevationTokens::Level2,
            pressed_elevation = ElevationTokens::Level2,
            focused_elevation = ElevationTokens::Level2,
            hovered_elevation = ElevationTokens::Level3,
        ),
        interaction_source = interaction_source,
        content = move || content(),
    );
}

// ---- floatingToolbarVerticalNestedScroll -------------------------------------------------------------------------

/// The modifier expanding and collapsing a toolbar as the content scrolls (Compose's member extension
/// `FloatingToolbarDefaults.floatingToolbarVerticalNestedScroll`; a method of every Modifier here, Rust having no
/// member extensions).
#[named(trait FloatingToolbarDefaultsModifiers)]
impl Modifier {
    /// Calls `on_collapse` once the content under it has scrolled forwards (up) `collapse_scroll_distance_threshold`
    /// past where it last expanded, `on_expand` once it has scrolled back `expand_scroll_distance_threshold` past
    /// where it last collapsed (Compose's `Modifier.floatingToolbarVerticalNestedScroll`); `expanded` is what the
    /// toolbar is now, `reverse_layout` says the content's scrolling is reversed. A nested scroll connection: put it
    /// above the scrolling content.
    ///
    /// Kotlin: `floatingToolbarVerticalNestedScroll(expanded: Boolean, onExpand: () -> Unit, onCollapse: () -> Unit, expandScrollDistanceThreshold: Dp = ScrollDistanceThreshold, collapseScrollDistanceThreshold: Dp = ScrollDistanceThreshold, reverseLayout: Boolean = false)`
    #[allow(clippy::too_many_arguments)]
    fn floating_toolbar_vertical_nested_scroll(
        self,
        expanded: bool,
        #[memoized] on_expand: impl Fn() + 'static,
        #[memoized] on_collapse: impl Fn() + 'static,
        #[default(FloatingToolbarDefaults::ScrollDistanceThreshold)] expand_scroll_distance_threshold: Dp,
        #[default(FloatingToolbarDefaults::ScrollDistanceThreshold)] collapse_scroll_distance_threshold: Dp,
        #[default(false)] reverse_layout: bool,
    ) -> Modifier {
        let (on_expand_id, on_collapse_id) = (on_expand.id(), on_collapse.id());
        self.then_element(VerticalNestedScrollExpansionElement {
            expanded,
            on_expand: Rc::new(on_expand.into_value()),
            on_expand_id,
            on_collapse: Rc::new(on_collapse.into_value()),
            on_collapse_id,
            reverse_layout,
            expand_scroll_threshold: expand_scroll_distance_threshold,
            collapse_scroll_threshold: collapse_scroll_distance_threshold,
        })
    }
}

/// Compose's internal `FloatingToolbarDefaults.VerticalNestedScrollExpansionElement`: equal for the same values and
/// the same callbacks (by their identity as memoized closures, Kotlin's `!==`).
struct VerticalNestedScrollExpansionElement {
    expanded: bool,
    on_expand: Rc<dyn Fn()>,
    on_expand_id: Option<u64>,
    on_collapse: Rc<dyn Fn()>,
    on_collapse_id: Option<u64>,
    reverse_layout: bool,
    expand_scroll_threshold: Dp,
    collapse_scroll_threshold: Dp,
}

impl PartialEq for VerticalNestedScrollExpansionElement {
    fn eq(&self, other: &Self) -> bool {
        self.expanded == other.expanded
            && self.reverse_layout == other.reverse_layout
            && self.on_expand_id.is_some()
            && self.on_expand_id == other.on_expand_id
            && self.on_collapse_id.is_some()
            && self.on_collapse_id == other.on_collapse_id
            && self.expand_scroll_threshold == other.expand_scroll_threshold
            && self.collapse_scroll_threshold == other.collapse_scroll_threshold
    }
}

impl ModifierNodeElement for VerticalNestedScrollExpansionElement {
    type Node = VerticalNestedScrollExpansionNode;

    fn create(&self) -> VerticalNestedScrollExpansionNode {
        VerticalNestedScrollExpansionNode {
            core: Rc::new(VerticalNestedScrollExpansionCore {
                expanded: Cell::new(self.expanded),
                on_expand: RefCell::new(self.on_expand.clone()),
                on_collapse: RefCell::new(self.on_collapse.clone()),
                reverse_layout: Cell::new(self.reverse_layout),
                expand_scroll_threshold: Cell::new(self.expand_scroll_threshold),
                collapse_scroll_threshold: Cell::new(self.collapse_scroll_threshold),
                expand_scroll_threshold_px: Cell::new(0.0),
                collapse_scroll_threshold_px: Cell::new(0.0),
                content_offset: Cell::new(0.0),
                threshold: Cell::new(0.0),
                reverse_layout_factor: Cell::new(if self.reverse_layout { -1.0 } else { 1.0 }),
                handle: RefCell::new(None),
            }),
            nested_scroll_node: None,
        }
    }

    fn update(&self, node: &mut VerticalNestedScrollExpansionNode) {
        node.core.update_node(
            self.expanded,
            self.on_expand.clone(),
            self.on_collapse.clone(),
            self.reverse_layout,
            self.expand_scroll_threshold,
            self.collapse_scroll_threshold,
        );
    }

    fn name(&self) -> &'static str {
        "floatingToolbarVerticalNestedScroll"
    }
}

/// Compose's internal `FloatingToolbarDefaults.VerticalNestedScrollExpansionNode`: a delegating node whose nested
/// scroll node's connection is what it shares ([`VerticalNestedScrollExpansionCore`]).
struct VerticalNestedScrollExpansionNode {
    core: Rc<VerticalNestedScrollExpansionCore>,
    nested_scroll_node: Option<DelegatedNode<NestedScrollNode>>,
}

impl ModifierNode for VerticalNestedScrollExpansionNode {
    fn on_attach(&mut self, cx: &NodeCx) {
        if self.nested_scroll_node.is_none() {
            let connection: Rc<dyn NestedScrollConnection> = self.core.clone();
            self.nested_scroll_node =
                Some(cx.delegate(kompoz_ui::input::nested_scroll_modifier_node(connection, None)));
        }
        *self.core.handle.borrow_mut() = Some(cx.handle());
        let density = cx.require_density();
        let core = &self.core;
        core.expand_scroll_threshold_px.set(density.to_px(core.expand_scroll_threshold.get()));
        core.collapse_scroll_threshold_px.set(density.to_px(core.collapse_scroll_threshold.get()));
        core.update_threshold();
    }

    fn should_auto_invalidate(&self) -> bool {
        false
    }
}

/// The node's values (Kotlin's `var`s of the node) and what it counts, shared with its nested scroll connection.
struct VerticalNestedScrollExpansionCore {
    expanded: Cell<bool>,
    on_expand: RefCell<Rc<dyn Fn()>>,
    on_collapse: RefCell<Rc<dyn Fn()>>,
    reverse_layout: Cell<bool>,
    expand_scroll_threshold: Cell<Dp>,
    collapse_scroll_threshold: Cell<Dp>,
    expand_scroll_threshold_px: Cell<f32>,
    collapse_scroll_threshold_px: Cell<f32>,
    content_offset: Cell<f32>,
    threshold: Cell<f32>,
    // In reverse layouts, scrolling direction is flipped. We will use this factor to flip some of the values we read
    // on the onPostScroll to ensure consistent behavior regardless of scroll direction.
    reverse_layout_factor: Cell<f32>,
    handle: RefCell<Option<NodeHandle>>,
}

impl VerticalNestedScrollExpansionCore {
    fn update_node(
        &self,
        expanded: bool,
        on_expand: Rc<dyn Fn()>,
        on_collapse: Rc<dyn Fn()>,
        reverse_layout: bool,
        expand_scroll_threshold: Dp,
        collapse_scroll_threshold: Dp,
    ) {
        if self.expand_scroll_threshold.get() != expand_scroll_threshold
            || self.collapse_scroll_threshold.get() != collapse_scroll_threshold
        {
            self.expand_scroll_threshold.set(expand_scroll_threshold);
            self.collapse_scroll_threshold.set(collapse_scroll_threshold);
            let density = self.handle.borrow().as_ref().and_then(|h| h.cx()).map(|cx| cx.require_density());
            if let Some(density) = density {
                self.expand_scroll_threshold_px.set(density.to_px(expand_scroll_threshold));
                self.collapse_scroll_threshold_px.set(density.to_px(collapse_scroll_threshold));
            }
            self.update_threshold();
        }
        if self.reverse_layout.get() != reverse_layout {
            self.reverse_layout.set(reverse_layout);
            self.reverse_layout_factor.set(if reverse_layout { -1.0 } else { 1.0 });
        }
        *self.on_expand.borrow_mut() = on_expand;
        *self.on_collapse.borrow_mut() = on_collapse;
        if self.expanded.get() != expanded {
            self.expanded.set(expanded);
            self.update_threshold();
        }
    }

    fn update_threshold(&self) {
        self.threshold.set(if self.expanded.get() {
            self.content_offset.get() - self.collapse_scroll_threshold_px.get()
        } else {
            self.content_offset.get() + self.expand_scroll_threshold_px.get()
        });
    }
}

impl NestedScrollConnection for VerticalNestedScrollExpansionCore {
    fn on_post_scroll(&self, consumed: Offset, _available: Offset, _source: NestedScrollSource) -> Offset {
        let scroll_delta = consumed.y * self.reverse_layout_factor.get();
        let content_offset = self.content_offset.get() + scroll_delta;
        self.content_offset.set(content_offset);
        if scroll_delta < 0.0 && content_offset <= self.threshold.get() {
            self.threshold.set(content_offset + self.expand_scroll_threshold_px.get());
            let on_collapse = self.on_collapse.borrow().clone();
            on_collapse();
        } else if scroll_delta > 0.0 && content_offset >= self.threshold.get() {
            self.threshold.set(content_offset - self.collapse_scroll_threshold_px.get());
            let on_expand = self.on_expand.borrow().clone();
            on_expand();
        }
        Offset::Zero
    }
}

// ---- FloatingToolbarColors ---------------------------------------------------------------------------------------

/// A floating toolbar's colors (Compose's Expressive `FloatingToolbarColors`): its container's and content's, and
/// its FAB's. Equal by its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct FloatingToolbarColors {
    pub toolbar_container_color: Color,
    pub toolbar_content_color: Color,
    pub fab_container_color: Color,
    pub fab_content_color: Color,
}

impl FloatingToolbarColors {
    /// Kotlin: `FloatingToolbarColors(toolbarContainerColor: Color, toolbarContentColor: Color, fabContainerColor: Color, fabContentColor: Color)`
    pub fn new(
        toolbar_container_color: Color,
        toolbar_content_color: Color,
        fab_container_color: Color,
        fab_content_color: Color,
    ) -> FloatingToolbarColors {
        FloatingToolbarColors { toolbar_container_color, toolbar_content_color, fab_container_color, fab_content_color }
    }
}

#[named]
impl FloatingToolbarColors {
    /// The same with each color given one other than unspecified (Compose's `copy`).
    pub fn copy(
        &self,
        #[default(self.toolbar_container_color)] toolbar_container_color: Color,
        #[default(self.toolbar_content_color)] toolbar_content_color: Color,
        #[default(self.fab_container_color)] fab_container_color: Color,
        #[default(self.fab_content_color)] fab_content_color: Color,
    ) -> FloatingToolbarColors {
        FloatingToolbarColors::new(
            toolbar_container_color.take_or_else(|| self.toolbar_container_color),
            toolbar_content_color.take_or_else(|| self.toolbar_content_color),
            fab_container_color.take_or_else(|| self.fab_container_color),
            fab_content_color.take_or_else(|| self.fab_content_color),
        )
    }
}

// ---- FAB positions -----------------------------------------------------------------------------------------------

/// Where a horizontal toolbar's FAB is (Compose's Expressive value class `FloatingToolbarHorizontalFabPosition`): at
/// its start or its end.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FloatingToolbarHorizontalFabPosition(i32);

impl FloatingToolbarHorizontalFabPosition {
    /// At the toolbar's start.
    pub const Start: FloatingToolbarHorizontalFabPosition = FloatingToolbarHorizontalFabPosition(0);
    /// At the toolbar's end (the default).
    pub const End: FloatingToolbarHorizontalFabPosition = FloatingToolbarHorizontalFabPosition(1);
}

/// Compose's toString.
impl fmt::Display for FloatingToolbarHorizontalFabPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            FloatingToolbarHorizontalFabPosition::Start => "FloatingToolbarHorizontalFabPosition.Start",
            _ => "FloatingToolbarHorizontalFabPosition.End",
        })
    }
}

impl fmt::Debug for FloatingToolbarHorizontalFabPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Where a vertical toolbar's FAB is (Compose's Expressive value class `FloatingToolbarVerticalFabPosition`): above
/// or below it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FloatingToolbarVerticalFabPosition(i32);

impl FloatingToolbarVerticalFabPosition {
    /// Above the toolbar.
    pub const Top: FloatingToolbarVerticalFabPosition = FloatingToolbarVerticalFabPosition(0);
    /// Below the toolbar (the default).
    pub const Bottom: FloatingToolbarVerticalFabPosition = FloatingToolbarVerticalFabPosition(1);
}

/// Compose's toString.
impl fmt::Display for FloatingToolbarVerticalFabPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            FloatingToolbarVerticalFabPosition::Top => "FloatingToolbarVerticalFabPosition.Top",
            _ => "FloatingToolbarVerticalFabPosition.Bottom",
        })
    }
}

impl fmt::Debug for FloatingToolbarVerticalFabPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// ---- FloatingToolbarState ----------------------------------------------------------------------------------------

/// A floating toolbar's state, saved and restored (Compose's Expressive `rememberFloatingToolbarState`).
#[kompozable]
pub fn remember_floating_toolbar_state(
    #[default(-f32::MAX)] initial_offset_limit: f32,
    #[default(0.0)] initial_offset: f32,
    #[default(0.0)] initial_content_offset: f32,
) -> Rc<dyn FloatingToolbarState> {
    remember_saveable_with(&floating_toolbar_state_saver(), move || {
        FloatingToolbarState(initial_offset_limit, initial_offset, initial_content_offset)
    })
}

/// Where a floating toolbar is moved by its scroll behavior (Compose's Expressive interface `FloatingToolbarState`):
/// its offset (0 at rest, down to its offset limit out of view) and how far the content has scrolled. Snapshot
/// state; shared as `Rc<dyn FloatingToolbarState>`.
pub trait FloatingToolbarState {
    /// How far the toolbar can be moved, a negative px value: as far as gets it out of view (Compose's
    /// `offsetLimit`).
    fn offset_limit(&self) -> f32;
    fn set_offset_limit(&self, offset_limit: f32);

    /// How far it is moved, a px value from 0 (at rest) to its offset limit (Compose's `offset`); a value set is
    /// kept within those.
    fn offset(&self) -> f32;
    fn set_offset(&self, offset: f32);

    /// How far the content has scrolled since it was last all the way back, px (Compose's `contentOffset`).
    fn content_offset(&self) -> f32;
    fn set_content_offset(&self, content_offset: f32);
}

impl PartialEq for dyn FloatingToolbarState {
    /// The same state (Kotlin's `==` of an interface's implementations: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// Saves a state's offset limit, offset and content offset (Compose's internal `FloatingToolbarState.Saver`).
pub(crate) fn floating_toolbar_state_saver() -> Saver<Rc<dyn FloatingToolbarState>> {
    list_saver(
        |it: &Rc<dyn FloatingToolbarState>| {
            use kompoz_runtime::Saveable;
            vec![it.offset_limit().save(), it.offset().save(), it.content_offset().save()]
        },
        |it| {
            use kompoz_runtime::Saveable;
            let [limit, offset, content] = it else { return None };
            Some(FloatingToolbarState(f32::restore(limit)?, f32::restore(offset)?, f32::restore(content)?))
        },
    )
}

/// A floating toolbar's state (Compose's Expressive function `FloatingToolbarState(…)`): in composition, rather
/// [`remember_floating_toolbar_state`].
///
/// Kotlin: `FloatingToolbarState(initialOffsetLimit: Float, initialOffset: Float, initialContentOffset: Float): FloatingToolbarState`
pub fn FloatingToolbarState(
    initial_offset_limit: f32,
    initial_offset: f32,
    initial_content_offset: f32,
) -> Rc<dyn FloatingToolbarState> {
    Rc::new(FloatingToolbarStateImpl {
        offset_limit: mutable_float_state_of(initial_offset_limit),
        offset: mutable_float_state_of(initial_offset),
        content_offset: mutable_float_state_of(initial_content_offset),
    })
}

/// Compose's private `FloatingToolbarStateImpl`: three float states.
struct FloatingToolbarStateImpl {
    offset_limit: MutableState<f32>,
    offset: MutableState<f32>,
    content_offset: MutableState<f32>,
}

impl FloatingToolbarState for FloatingToolbarStateImpl {
    fn offset_limit(&self) -> f32 {
        self.offset_limit.get()
    }

    fn set_offset_limit(&self, offset_limit: f32) {
        self.offset_limit.set(offset_limit);
    }

    fn offset(&self) -> f32 {
        self.offset.get()
    }

    /// (Kotlin's `coerceIn(minimumValue = offsetLimit, maximumValue = 0f)`: a positive limit throws there, panics
    /// here.)
    #[track_caller]
    fn set_offset(&self, offset: f32) {
        let limit = self.offset_limit.get();
        if limit > 0.0 {
            kompoz_runtime::kt::panic_kt(
                "IllegalArgumentException",
                &format!("Cannot coerce value to an empty range: maximum 0.0 is less than minimum {limit:?}."),
            );
        }
        self.offset.set(offset.clamp(limit, 0.0));
    }

    fn content_offset(&self) -> f32 {
        self.content_offset.get()
    }

    fn set_content_offset(&self, content_offset: f32) {
        self.content_offset.set(content_offset);
    }
}

/// Settles a toolbar let go between at rest and out of view (Compose's private `settleFloatingToolbar`): first
/// carries on a fling's `velocity` along `fling_animation_spec`, then, still between, animates it to the nearer end
/// along `snap_animation_spec`; the velocity left over.
async fn settle_floating_toolbar(
    state: Rc<dyn FloatingToolbarState>,
    velocity: f32,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    fling_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
) -> Velocity {
    // Check if the toolbar is completely collapsed/expanded. If so, no need to settle the toolbar, and just return
    // Zero Velocity. Note that we don't check for 0f due to float precision with the collapsedFraction calculation.
    let fraction = collapsed_fraction(&*state);
    if fraction < 0.01 || fraction == 1.0 {
        return Velocity::Zero;
    }
    let remaining_velocity = Rc::new(Cell::new(velocity));
    // In case there is an initial velocity that was left after a previous user fling, animate to continue the motion
    // to expand or collapse the toolbar.
    if velocity.abs() > 1.0 {
        let last_value = Cell::new(0.0f32);
        let (fling_state, remaining) = (state.clone(), remaining_velocity.clone());
        let block = Box::new(move |scope: &AnimationScope<f32, AnimationVector1D>| {
            let delta = scope.value() - last_value.get();
            let initial_offset = fling_state.offset();
            fling_state.set_offset(initial_offset + delta);
            let consumed = (initial_offset - fling_state.offset()).abs();
            last_value.set(scope.value());
            remaining.set(scope.velocity());
            // avoid rounding errors and stop if anything is unconsumed
            if (delta - consumed).abs() > 0.5 {
                scope.cancel_animation();
            }
        });
        let animation = named_args!(float_animation_state(0.0, initial_velocity = velocity));
        named_args!(animation.animate_decay(fling_animation_spec, block = block)).await;
    }
    if state.offset() < 0.0 && state.offset() > state.offset_limit() {
        let target = if collapsed_fraction(&*state) < 0.5 { 0.0 } else { state.offset_limit() };
        let snap_state = state.clone();
        let block =
            Box::new(move |scope: &AnimationScope<f32, AnimationVector1D>| snap_state.set_offset(scope.value()));
        let animation = named_args!(float_animation_state(state.offset()));
        named_args!(animation.animate_to(target, animation_spec = snap_animation_spec, block = block)).await;
    }
    Velocity::new(0.0, remaining_velocity.get())
}

/// How far out of view it is, 0 to 1 (Compose's private `FloatingToolbarState.collapsedFraction()`).
fn collapsed_fraction(state: &dyn FloatingToolbarState) -> f32 {
    if state.offset_limit() != 0.0 { state.offset() / state.offset_limit() } else { 0.0 }
}

// ---- FloatingToolbarExitDirection --------------------------------------------------------------------------------

/// The way a floating toolbar leaves the screen as the content scrolls (Compose's Expressive value class
/// `FloatingToolbarExitDirection`): down, up, towards the start or the end.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FloatingToolbarExitDirection(i32);

impl FloatingToolbarExitDirection {
    /// Down, out of the bottom.
    pub const Bottom: FloatingToolbarExitDirection = FloatingToolbarExitDirection(0);
    /// Up, out of the top.
    pub const Top: FloatingToolbarExitDirection = FloatingToolbarExitDirection(1);
    /// Out of the start.
    pub const Start: FloatingToolbarExitDirection = FloatingToolbarExitDirection(2);
    /// Out of the end.
    pub const End: FloatingToolbarExitDirection = FloatingToolbarExitDirection(3);
}

/// Compose's toString.
impl fmt::Display for FloatingToolbarExitDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            FloatingToolbarExitDirection::Bottom => "FloatingToolbarExitDirection.Bottom",
            FloatingToolbarExitDirection::Top => "FloatingToolbarExitDirection.Top",
            FloatingToolbarExitDirection::Start => "FloatingToolbarExitDirection.Start",
            _ => "FloatingToolbarExitDirection.End",
        })
    }
}

impl fmt::Debug for FloatingToolbarExitDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// ---- The layouts -------------------------------------------------------------------------------------------------

/// The toolbar's modifiers before its background: `modifier`, its scroll behavior's, then a layer casting
/// `shadow_elevation`'s shadow of `shape`, clipped to it.
fn toolbar_container(
    modifier: Modifier,
    scroll_behavior: Option<&Rc<dyn FloatingToolbarScrollBehavior>>,
    shadow_elevation: State<Dp>,
    shape: Arc<dyn Shape>,
) -> Modifier {
    let behavior = match scroll_behavior {
        Some(behavior) => behavior.floating_scroll_behavior(Modifier),
        None => Modifier,
    };
    modifier.then(behavior).graphics_layer_block(move |layer: &mut GraphicsLayerScope| {
        layer.shadow_elevation = layer.density().to_px(shadow_elevation.value());
        layer.shape = shape.clone();
        layer.clip = true;
    })
}

/// Compose's private `HorizontalFloatingToolbarLayout`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn HorizontalFloatingToolbarLayout(
    modifier: Modifier,
    expanded: bool,
    on_a11y_force_collapse: Rc<dyn Fn(bool)>,
    colors: FloatingToolbarColors,
    content_padding: PaddingValues,
    scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    shape: Arc<dyn Shape>,
    leading_content: Option<Rc<dyn Fn()>>,
    trailing_content: Option<Rc<dyn Fn()>>,
    expanded_shadow_elevation: Dp,
    collapsed_shadow_elevation: Dp,
    content: Rc<dyn Fn()>,
) {
    let expand_toolbar_action_label = get_string(Strings::FloatingToolbarExpand);
    let collapse_toolbar_action_label = get_string(Strings::FloatingToolbarCollapse);
    let expanded_state = remember_updated_state!(expanded);
    let shadow_elevation_state = animate_dp_as_state(
        if expanded { expanded_shadow_elevation } else { collapsed_shadow_elevation },
        animation_spec = FloatingToolbarDefaults.animation_spec(),
    );
    let has_leading = leading_content.is_some();
    let has_trailing = trailing_content.is_some();
    // Ensures this motion will cause the padding to bounce.
    let padding_spec: Arc<dyn AnimationSpec<f32>> = MaterialTheme::motion_scheme().default_effects_spec::<f32>();
    Row(
        modifier = toolbar_container(modifier, scroll_behavior.as_ref(), shadow_elevation_state, shape.clone())
            .height_in_with(FloatingToolbarDefaults::ContainerSize, Dp::Unspecified)
            .background(colors.toolbar_container_color, shape)
            .padding_values(content_padding),
        horizontal_arrangement = Arrangement::Center,
        vertical_alignment = Alignment::CenterVertically,
        content = move || {
            let (leading, trailing, content) = (leading_content.clone(), trailing_content.clone(), content.clone());
            let (expanded_state, on_a11y) = (expanded_state.clone(), on_a11y_force_collapse.clone());
            let (expand_label, collapse_label) =
                (expand_toolbar_action_label.clone(), collapse_toolbar_action_label.clone());
            let padding_spec = padding_spec.clone();
            KompozitionLocalProvider(&[LocalContentColor.provides(colors.toolbar_content_color)], move || {
                if let Some(it) = leading.clone() {
                    RowAnimatedVisibility(
                        visible = expanded_state.value(),
                        enter = FloatingToolbarDefaults.horizontal_enter_transition(Alignment::Start),
                        exit = FloatingToolbarDefaults.horizontal_exit_transition(Alignment::End),
                        content = move |_| {
                            let it = it.clone();
                            Row(content = move || it());
                        },
                    );
                }
                let content = content.clone();
                Row(
                    modifier = Modifier
                        .parent_semantics(toolbar_actions(
                            expanded_state.clone(),
                            on_a11y.clone(),
                            expand_label.clone(),
                            collapse_label.clone(),
                        ))
                        .minimum_interactive_balanced_padding(
                            expanded && has_leading,
                            expanded && has_trailing,
                            padding_spec.clone(),
                        ),
                    content = move || content(),
                );
                if let Some(it) = trailing.clone() {
                    RowAnimatedVisibility(
                        visible = expanded_state.value(),
                        enter = FloatingToolbarDefaults.horizontal_enter_transition(Alignment::End),
                        exit = FloatingToolbarDefaults.horizontal_exit_transition(Alignment::Start),
                        content = move |_| {
                            let it = it.clone();
                            Row(content = move || it());
                        },
                    );
                }
            });
        },
    );
}

/// The parent semantics giving a screen reader the action expanding or collapsing the toolbar (the `parentSemantics
/// { customActions = customToolbarActions(…) }` of Compose's layouts).
fn toolbar_actions(
    expanded_state: State<bool>,
    on_a11y_force_collapse: Rc<dyn Fn(bool)>,
    expand_action_label: String,
    collapse_action_label: String,
) -> impl Fn(&mut kompoz_ui::semantics::SemanticsConfiguration) + 'static {
    move |config| {
        let (expand, collapse) = (on_a11y_force_collapse.clone(), on_a11y_force_collapse.clone());
        config.set_custom_actions(custom_toolbar_actions(
            expanded_state.value(),
            move || {
                expand(false);
                true
            },
            move || {
                collapse(true);
                true
            },
            expand_action_label.clone(),
            collapse_action_label.clone(),
        ));
    }
}

/// Compose's private `HorizontalFloatingToolbarWithFabLayout`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn HorizontalFloatingToolbarWithFabLayout(
    modifier: Modifier,
    expanded: bool,
    on_a11y_force_collapse: Rc<dyn Fn(bool)>,
    colors: FloatingToolbarColors,
    toolbar_to_fab_gap: Dp,
    toolbar_content_padding: PaddingValues,
    scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    toolbar_shape: Arc<dyn Shape>,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    fab: Rc<dyn Fn()>,
    fab_position: FloatingToolbarHorizontalFabPosition,
    expanded_shadow_elevation: Dp,
    collapsed_shadow_elevation: Dp,
    toolbar: Rc<dyn Fn()>,
) {
    let expand_transition = update_transition(if expanded { 1.0f32 } else { 0.0 }, Some("expanded state".to_string()));
    let expanded_progress = named_args!(
        expand_transition
            .animate_float(transition_spec = move |_: &Segment<f32>| animation_spec.clone(), |it: &f32| *it)
    );
    let expand_toolbar_action_label = get_string(Strings::FloatingToolbarExpand);
    let collapse_toolbar_action_label = get_string(Strings::FloatingToolbarCollapse);
    let expanded_state = remember_updated_state!(expanded);
    let behavior = match &scroll_behavior {
        Some(behavior) => behavior.floating_scroll_behavior(Modifier),
        None => Modifier,
    };
    let progress = expanded_progress.clone();
    Layout(
        content = move || {
            let toolbar = toolbar.clone();
            Row(
                modifier = Modifier
                    .background(colors.toolbar_container_color)
                    .padding_values(toolbar_content_padding.clone())
                    .horizontal_scroll(&remember_scroll_state()),
                vertical_alignment = Alignment::CenterVertically,
                content = move || {
                    KompozitionLocalProvider(&[LocalContentColor.provides(colors.toolbar_content_color)], &*toolbar);
                },
            );
            let fab = fab.clone();
            Box(
                modifier = Modifier.parent_semantics(toolbar_actions(
                    expanded_state.clone(),
                    on_a11y_force_collapse.clone(),
                    expand_toolbar_action_label.clone(),
                    collapse_toolbar_action_label.clone(),
                )),
                content = move || fab(),
            );
        },
        modifier = modifier
            .default_min_size_with(Dp::Unspecified, *FloatingToolbarDefaults::FabSizeRange.end())
            .then(behavior),
        measure_policy = move |scope: &MeasureScope, measurables: &[&dyn Measurable], constraints: Constraints| {
            let (toolbar_measurable, fab_measurable) = (measurables[0], measurables[1]);
            let progress = progress.value();
            // The FAB is in its smallest size when the expanded progress is 1f.
            let fab_size_constraint =
                scope.round_to_px(lerp_range(&FloatingToolbarDefaults::FabSizeRange, 1.0 - progress));
            let fab_placeable = fab_measurable.measure(Constraints::new(
                fab_size_constraint,
                fab_size_constraint,
                fab_size_constraint,
                fab_size_constraint,
            ));
            // Compute the toolbar's max intrinsic width. We will use it as a base to determine the actual width with
            // the animation progress and the total layout width.
            let max_toolbar_placeable_width =
                toolbar_measurable.max_intrinsic_width(scope.round_to_px(FloatingToolbarDefaults::ContainerSize));
            let target_width = (max_toolbar_placeable_width as f32 * progress) as i32;
            let target_height = scope.round_to_px(FloatingToolbarDefaults::ContainerSize);
            // Constraint the toolbar to the available width while taking into account the FAB width.
            let toolbar_placeable = toolbar_measurable.measure(Constraints::new(
                constraints.min_width,
                target_width.clamp(0, constraints.max_width),
                target_height.clamp(0, constraints.min_height),
                constraints.max_height,
            ));
            let width = max_toolbar_placeable_width
                + scope.round_to_px(toolbar_to_fab_gap)
                + scope.round_to_px(*FloatingToolbarDefaults::FabSizeRange.start());
            let height = constraints.min_height;
            let toolbar_top_offset = (height - toolbar_placeable.height()) / 2;
            let fab_top_offset = (height - fab_placeable.height()) / 2;
            let fab_x = if fab_position == FloatingToolbarHorizontalFabPosition::End {
                width - fab_placeable.width()
            } else {
                0
            };
            let toolbar_x = if fab_position == FloatingToolbarHorizontalFabPosition::End {
                max_toolbar_placeable_width - toolbar_placeable.width()
            } else {
                width - max_toolbar_placeable_width
            };
            let toolbar_shape = toolbar_shape.clone();
            named_args!(scope.layout(width, height, move |p| {
                let toolbar_elevation =
                    lerp_dp(collapsed_shadow_elevation, expanded_shadow_elevation, progress.min(1.0));
                let toolbar_shape = toolbar_shape.clone();
                p.place_relative_with_layer(
                    &toolbar_placeable,
                    toolbar_x,
                    toolbar_top_offset,
                    0.0,
                    move |layer: &mut GraphicsLayerScope| {
                        layer.shadow_elevation = layer.density().to_px(toolbar_elevation);
                        layer.shape = toolbar_shape.clone();
                        layer.clip = true;
                    },
                );
                p.place_relative(&fab_placeable, fab_x, fab_top_offset, 0.0);
            }))
        },
    );
}

/// Compose's private `VerticalFloatingToolbarLayout`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn VerticalFloatingToolbarLayout(
    modifier: Modifier,
    expanded: bool,
    on_a11y_force_collapse: Rc<dyn Fn(bool)>,
    colors: FloatingToolbarColors,
    content_padding: PaddingValues,
    scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    shape: Arc<dyn Shape>,
    leading_content: Option<Rc<dyn Fn()>>,
    trailing_content: Option<Rc<dyn Fn()>>,
    expanded_shadow_elevation: Dp,
    collapsed_shadow_elevation: Dp,
    content: Rc<dyn Fn()>,
) {
    let expand_toolbar_action_label = get_string(Strings::FloatingToolbarExpand);
    let collapse_toolbar_action_label = get_string(Strings::FloatingToolbarCollapse);
    let expanded_state = remember_updated_state!(expanded);
    let shadow_elevation_state = animate_dp_as_state(
        if expanded { expanded_shadow_elevation } else { collapsed_shadow_elevation },
        animation_spec = FloatingToolbarDefaults.animation_spec(),
    );
    let has_leading = leading_content.is_some();
    let has_trailing = trailing_content.is_some();
    // Ensures this motion will cause the padding to bounce.
    let padding_spec: Arc<dyn AnimationSpec<f32>> = MaterialTheme::motion_scheme().default_effects_spec::<f32>();
    Column(
        modifier = toolbar_container(modifier, scroll_behavior.as_ref(), shadow_elevation_state, shape.clone())
            .width_in_with(FloatingToolbarDefaults::ContainerSize, Dp::Unspecified)
            .background(colors.toolbar_container_color, shape)
            .padding_values(content_padding),
        vertical_arrangement = Arrangement::Center,
        horizontal_alignment = Alignment::CenterHorizontally,
        content = move || {
            let (leading, trailing, content) = (leading_content.clone(), trailing_content.clone(), content.clone());
            let (expanded_state, on_a11y) = (expanded_state.clone(), on_a11y_force_collapse.clone());
            let (expand_label, collapse_label) =
                (expand_toolbar_action_label.clone(), collapse_toolbar_action_label.clone());
            let padding_spec = padding_spec.clone();
            KompozitionLocalProvider(&[LocalContentColor.provides(colors.toolbar_content_color)], move || {
                if let Some(it) = leading.clone() {
                    ColumnAnimatedVisibility(
                        visible = expanded_state.value(),
                        enter = FloatingToolbarDefaults.vertical_enter_transition(Alignment::Bottom),
                        exit = FloatingToolbarDefaults.vertical_exit_transition(Alignment::Bottom),
                        content = move |_| {
                            let it = it.clone();
                            Column(content = move || it());
                        },
                    );
                }
                let content = content.clone();
                Column(
                    modifier = Modifier
                        .parent_semantics(toolbar_actions(
                            expanded_state.clone(),
                            on_a11y.clone(),
                            expand_label.clone(),
                            collapse_label.clone(),
                        ))
                        .minimum_interactive_balanced_padding(
                            expanded && has_leading,
                            expanded && has_trailing,
                            padding_spec.clone(),
                        ),
                    content = move || content(),
                );
                if let Some(it) = trailing.clone() {
                    ColumnAnimatedVisibility(
                        visible = expanded_state.value(),
                        enter = FloatingToolbarDefaults.vertical_enter_transition(Alignment::Top),
                        exit = FloatingToolbarDefaults.vertical_exit_transition(Alignment::Top),
                        content = move |_| {
                            let it = it.clone();
                            Column(content = move || it());
                        },
                    );
                }
            });
        },
    );
}

/// Compose's private `VerticalFloatingToolbarWithFabLayout`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn VerticalFloatingToolbarWithFabLayout(
    modifier: Modifier,
    expanded: bool,
    on_a11y_force_collapse: Rc<dyn Fn(bool)>,
    colors: FloatingToolbarColors,
    toolbar_to_fab_gap: Dp,
    toolbar_content_padding: PaddingValues,
    scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    toolbar_shape: Arc<dyn Shape>,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    fab: Rc<dyn Fn()>,
    fab_position: FloatingToolbarVerticalFabPosition,
    expanded_shadow_elevation: Dp,
    collapsed_shadow_elevation: Dp,
    toolbar: Rc<dyn Fn()>,
) {
    let expand_transition = update_transition(if expanded { 1.0f32 } else { 0.0 }, Some("expanded state".to_string()));
    let expanded_progress = named_args!(
        expand_transition
            .animate_float(transition_spec = move |_: &Segment<f32>| animation_spec.clone(), |it: &f32| *it)
    );
    let expand_toolbar_action_label = get_string(Strings::FloatingToolbarExpand);
    let collapse_toolbar_action_label = get_string(Strings::FloatingToolbarCollapse);
    let expanded_state = remember_updated_state!(expanded);
    let behavior = match &scroll_behavior {
        Some(behavior) => behavior.floating_scroll_behavior(Modifier),
        None => Modifier,
    };
    let progress = expanded_progress.clone();
    Layout(
        content = move || {
            let toolbar = toolbar.clone();
            Column(
                modifier = Modifier
                    .background(colors.toolbar_container_color)
                    .padding_values(toolbar_content_padding.clone())
                    .vertical_scroll(&remember_scroll_state()),
                horizontal_alignment = Alignment::CenterHorizontally,
                content = move || {
                    KompozitionLocalProvider(&[LocalContentColor.provides(colors.toolbar_content_color)], &*toolbar);
                },
            );
            let fab = fab.clone();
            Box(
                modifier = Modifier.parent_semantics(toolbar_actions(
                    expanded_state.clone(),
                    on_a11y_force_collapse.clone(),
                    expand_toolbar_action_label.clone(),
                    collapse_toolbar_action_label.clone(),
                )),
                content = move || fab(),
            );
        },
        modifier = modifier
            .default_min_size_with(*FloatingToolbarDefaults::FabSizeRange.end(), Dp::Unspecified)
            .then(behavior),
        measure_policy = move |scope: &MeasureScope, measurables: &[&dyn Measurable], constraints: Constraints| {
            let (toolbar_measurable, fab_measurable) = (measurables[0], measurables[1]);
            let progress = progress.value();
            // The FAB is in its smallest size when the expanded progress is 1f.
            let fab_size_constraint =
                scope.round_to_px(lerp_range(&FloatingToolbarDefaults::FabSizeRange, 1.0 - progress));
            let fab_placeable = fab_measurable.measure(Constraints::new(
                fab_size_constraint,
                fab_size_constraint,
                fab_size_constraint,
                fab_size_constraint,
            ));
            // Compute the toolbar's max intrinsic height. We will use it as a base to determine the actual height
            // with the animation progress and the total layout height.
            let max_toolbar_placeable_height =
                toolbar_measurable.max_intrinsic_height(scope.round_to_px(FloatingToolbarDefaults::ContainerSize));
            let target_height = (max_toolbar_placeable_height as f32 * progress).max(0.0) as i32;
            let target_width = scope.round_to_px(FloatingToolbarDefaults::ContainerSize);
            // Constraint the toolbar to the available height while taking into account the FAB height.
            let toolbar_placeable = toolbar_measurable.measure(Constraints::new(
                target_width.clamp(0, constraints.min_width),
                constraints.max_width,
                constraints.min_height,
                target_height.clamp(0, constraints.max_height),
            ));
            let width = constraints.min_width;
            let height = max_toolbar_placeable_height
                + scope.round_to_px(toolbar_to_fab_gap)
                + scope.round_to_px(*FloatingToolbarDefaults::FabSizeRange.start());
            let toolbar_edge_offset = (width - toolbar_placeable.width()) / 2;
            let fab_edge_offset = (width - fab_placeable.width()) / 2;
            let fab_y = if fab_position == FloatingToolbarVerticalFabPosition::Bottom {
                height - fab_placeable.height()
            } else {
                0
            };
            let toolbar_y = if fab_position == FloatingToolbarVerticalFabPosition::Bottom {
                max_toolbar_placeable_height - toolbar_placeable.height()
            } else {
                height - max_toolbar_placeable_height
            };
            let toolbar_shape = toolbar_shape.clone();
            named_args!(scope.layout(width, height, move |p| {
                let toolbar_elevation =
                    lerp_dp(collapsed_shadow_elevation, expanded_shadow_elevation, progress.min(1.0));
                let toolbar_shape = toolbar_shape.clone();
                p.place_relative_with_layer(
                    &toolbar_placeable,
                    toolbar_edge_offset,
                    toolbar_y,
                    0.0,
                    move |layer: &mut GraphicsLayerScope| {
                        layer.shadow_elevation = layer.density().to_px(toolbar_elevation);
                        layer.shape = toolbar_shape.clone();
                        layer.clip = true;
                    },
                );
                p.place_relative(&fab_placeable, fab_edge_offset, fab_y, 0.0);
            }))
        },
    );
}

// ---- minimumInteractiveBalancedPadding ---------------------------------------------------------------------------

/// Compose's private `Modifier.minimumInteractiveBalancedPadding`.
trait MinimumInteractiveBalancedPaddingModifier {
    /// Pads the content so the room its minimum interactive sizes leave around it is the same across and down while
    /// either side content is away (animating along `animation_spec`).
    fn minimum_interactive_balanced_padding(
        self,
        has_visible_leading_content: bool,
        has_visible_trailing_content: bool,
        animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> Modifier;
}

impl MinimumInteractiveBalancedPaddingModifier for Modifier {
    fn minimum_interactive_balanced_padding(
        self,
        has_visible_leading_content: bool,
        has_visible_trailing_content: bool,
        animation_spec: Arc<dyn AnimationSpec<f32>>,
    ) -> Modifier {
        self.then_element(MinimumInteractiveBalancedPaddingElement {
            has_visible_leading_content,
            has_visible_trailing_content,
            animation_spec,
        })
    }
}

/// Compose's private data class `MinimumInteractiveBalancedPaddingElement`: equal for the same values (the spec by
/// Compose's equals).
struct MinimumInteractiveBalancedPaddingElement {
    has_visible_leading_content: bool,
    has_visible_trailing_content: bool,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
}

impl PartialEq for MinimumInteractiveBalancedPaddingElement {
    fn eq(&self, other: &Self) -> bool {
        self.has_visible_leading_content == other.has_visible_leading_content
            && self.has_visible_trailing_content == other.has_visible_trailing_content
            && *self.animation_spec == *other.animation_spec
    }
}

impl ModifierNodeElement for MinimumInteractiveBalancedPaddingElement {
    type Node = MinimumInteractiveBalancedPaddingNode;

    fn create(&self) -> MinimumInteractiveBalancedPaddingNode {
        let either = self.has_visible_leading_content || self.has_visible_trailing_content;
        MinimumInteractiveBalancedPaddingNode {
            has_visible_leading_content: self.has_visible_leading_content,
            has_visible_trailing_content: self.has_visible_trailing_content,
            animation_spec: self.animation_spec.clone(),
            padding_animation: Animatable((if either { 0.0f32 } else { 1.0 }, f32::VectorConverter)),
            handle: None,
        }
    }

    fn update(&self, node: &mut MinimumInteractiveBalancedPaddingNode) {
        node.has_visible_leading_content = self.has_visible_leading_content;
        node.has_visible_trailing_content = self.has_visible_trailing_content;
        node.animation_spec = self.animation_spec.clone();
        node.update_animation();
    }

    fn name(&self) -> &'static str {
        "minimumInteractiveBalancedPadding"
    }
}

/// Compose's private `MinimumInteractiveBalancedPaddingNode`.
struct MinimumInteractiveBalancedPaddingNode {
    has_visible_leading_content: bool,
    has_visible_trailing_content: bool,
    animation_spec: Arc<dyn AnimationSpec<f32>>,
    padding_animation: Animatable<f32, AnimationVector1D>,
    handle: Option<NodeHandle>,
}

impl MinimumInteractiveBalancedPaddingNode {
    fn update_animation(&self) {
        let Some(scope) = self.handle.as_ref().and_then(NodeHandle::coroutine_scope) else { return };
        let target = if !(self.has_visible_leading_content || self.has_visible_trailing_content) { 1.0 } else { 0.0 };
        let animation =
            named_args!(self.padding_animation.animate_to(target, animation_spec = self.animation_spec.clone()));
        scope.launch(async move {
            animation.await;
        });
    }
}

impl ModifierNode for MinimumInteractiveBalancedPaddingNode {
    node_capabilities!(layout);

    fn on_attach(&mut self, cx: &NodeCx) {
        self.handle = Some(cx.handle());
    }

    fn on_detach(&mut self, _: &NodeCx) {
        self.handle = None;
    }
}

impl LayoutModifierNode for MinimumInteractiveBalancedPaddingNode {
    fn measure(
        &mut self,
        scope: &MeasureScope,
        measurable: &dyn Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        let placeable = measurable.measure(constraints);
        let mut vertical_alignment_offset = 0.0f32;
        let mut horizontal_alignment_offset = 0.0f32;
        // Resolve the top and left paddings from the alignment lines whenever either of the leading or trailing
        // content is missing.
        if !self.has_visible_leading_content || !self.has_visible_trailing_content {
            let progress = self.padding_animation.value();
            let unspecified = AlignmentLine::Unspecified;
            vertical_alignment_offset = match placeable.get(MinimumInteractiveTopAlignmentLine) {
                it if it != unspecified => it as f32 * progress,
                _ => 0.0,
            };
            horizontal_alignment_offset = match placeable.get(MinimumInteractiveLeftAlignmentLine) {
                it if it != unspecified => it as f32 * progress,
                _ => 0.0,
            };
        }
        // Add padding to balance the alignment by ensuring that the horizontal and vertical alignment offsets are
        // visually similar. In case the vertical alignment offset is greater than the horizontal alignment offset,
        // we add additional horizontal padding to balance the paddings.
        let total_width = placeable.width()
            + ((vertical_alignment_offset - horizontal_alignment_offset) * 2.0).max(0.0).round_to_int();
        // In case the horizontal alignment offset is greater than the vertical alignment offset, we add additional
        // vertical padding to balance the paddings.
        let total_height = placeable.height()
            + ((horizontal_alignment_offset - vertical_alignment_offset) * 2.0).max(0.0).round_to_int();
        let (x, y) = ((total_width - placeable.width()) / 2, (total_height - placeable.height()) / 2);
        named_args!(scope.layout(total_width, total_height, move |p| p.place(&placeable, x, y, 0.0)))
    }
}

/// The screen reader's action of a toolbar: collapse it when `expanded`, else expand it (Compose's private
/// `customToolbarActions`).
fn custom_toolbar_actions(
    expanded: bool,
    expand_action: impl Fn() -> bool + 'static,
    collapse_action: impl Fn() -> bool + 'static,
    expand_action_label: String,
    collapse_action_label: String,
) -> Vec<CustomAccessibilityAction> {
    vec![if expanded {
        CustomAccessibilityAction::new(collapse_action_label, collapse_action)
    } else {
        CustomAccessibilityAction::new(expand_action_label, expand_action)
    }]
}

/// Compose's private `ClosedRange<Dp>.lerp(progress)`.
fn lerp_range(range: &RangeInclusive<Dp>, progress: f32) -> Dp {
    lerp_dp(*range.start(), *range.end(), progress)
}

/// Whether touch exploration is on (Compose's private `rememberTouchExplorationService`).
#[kompozable]
fn remember_touch_exploration_service() -> State<bool> {
    remember_accessibility_service_state(true, false, false)
}

// ---- The overrides -----------------------------------------------------------------------------------------------

/// The horizontal floating toolbar of an override (Compose's experimental `HorizontalFloatingToolbarOverride`): what
/// makes every [`HorizontalFloatingToolbar`] below a [`LocalHorizontalFloatingToolbarOverride`] providing it.
pub trait HorizontalFloatingToolbarOverride {
    /// Composes the toolbar of `scope`'s arguments (Compose's
    /// `HorizontalFloatingToolbarOverrideScope.HorizontalFloatingToolbar()`).
    fn HorizontalFloatingToolbar(&self, scope: &HorizontalFloatingToolbarOverrideScope);
}

impl PartialEq for dyn HorizontalFloatingToolbarOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The arguments of a [`HorizontalFloatingToolbar`], given to its override (Compose's experimental
/// `HorizontalFloatingToolbarOverrideScope`).
#[derive(Clone)]
pub struct HorizontalFloatingToolbarOverrideScope {
    is_expanded: bool,
    modifier: Modifier,
    colors: FloatingToolbarColors,
    content_padding: PaddingValues,
    scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    shape: Arc<dyn Shape>,
    leading_content: Option<Rc<dyn Fn()>>,
    trailing_content: Option<Rc<dyn Fn()>>,
    expanded_shadow_elevation: Dp,
    collapsed_shadow_elevation: Dp,
    content: Rc<dyn Fn()>,
}

impl HorizontalFloatingToolbarOverrideScope {
    /// Whether the toolbar is expanded (Compose's `isExpanded`).
    pub fn is_expanded(&self) -> bool {
        self.is_expanded
    }
    /// Compose's `modifier`.
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }
    /// Compose's `colors`.
    pub fn colors(&self) -> FloatingToolbarColors {
        self.colors
    }
    /// Compose's `contentPadding`.
    pub fn content_padding(&self) -> PaddingValues {
        self.content_padding.clone()
    }
    /// Compose's `scrollBehavior`.
    pub fn scroll_behavior(&self) -> Option<Rc<dyn FloatingToolbarScrollBehavior>> {
        self.scroll_behavior.clone()
    }
    /// Compose's `shape`.
    pub fn shape(&self) -> Arc<dyn Shape> {
        self.shape.clone()
    }
    /// Compose's `leadingContent`.
    pub fn leading_content(&self) -> Option<Rc<dyn Fn()>> {
        self.leading_content.clone()
    }
    /// Compose's `trailingContent`.
    pub fn trailing_content(&self) -> Option<Rc<dyn Fn()>> {
        self.trailing_content.clone()
    }
    /// Compose's `expandedShadowElevation`.
    pub fn expanded_shadow_elevation(&self) -> Dp {
        self.expanded_shadow_elevation
    }
    /// Compose's `collapsedShadowElevation`.
    pub fn collapsed_shadow_elevation(&self) -> Dp {
        self.collapsed_shadow_elevation
    }
    /// Compose's `content`.
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the horizontal floating toolbars below (Compose's experimental
/// `LocalHorizontalFloatingToolbarOverride`): [`DefaultHorizontalFloatingToolbarOverride`] unless provided.
pub static LocalHorizontalFloatingToolbarOverride: KompozitionLocal<Rc<dyn HorizontalFloatingToolbarOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultHorizontalFloatingToolbarOverride));

/// The horizontal floating toolbar with a FAB of an override (Compose's experimental
/// `HorizontalFloatingToolbarWithFabOverride`).
pub trait HorizontalFloatingToolbarWithFabOverride {
    /// Composes the toolbar of `scope`'s arguments (Compose's
    /// `HorizontalFloatingToolbarWithFabOverrideScope.HorizontalFloatingToolbarWithFab()`).
    fn HorizontalFloatingToolbarWithFab(&self, scope: &HorizontalFloatingToolbarWithFabOverrideScope);
}

impl PartialEq for dyn HorizontalFloatingToolbarWithFabOverride {
    /// The same override (by identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The arguments of a [`HorizontalFloatingToolbarWithFab`], given to its override (Compose's experimental
/// `HorizontalFloatingToolbarWithFabOverrideScope`).
#[derive(Clone)]
pub struct HorizontalFloatingToolbarWithFabOverrideScope {
    is_expanded: bool,
    floating_action_button: Rc<dyn Fn()>,
    modifier: Modifier,
    colors: FloatingToolbarColors,
    content_padding: PaddingValues,
    scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    shape: Arc<dyn Shape>,
    floating_action_button_position: FloatingToolbarHorizontalFabPosition,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    expanded_shadow_elevation: Dp,
    collapsed_shadow_elevation: Dp,
    content: Rc<dyn Fn()>,
}

impl HorizontalFloatingToolbarWithFabOverrideScope {
    /// Compose's `isExpanded`.
    pub fn is_expanded(&self) -> bool {
        self.is_expanded
    }
    /// Compose's `floatingActionButton`.
    pub fn floating_action_button(&self) -> Rc<dyn Fn()> {
        self.floating_action_button.clone()
    }
    /// Compose's `modifier`.
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }
    /// Compose's `colors`.
    pub fn colors(&self) -> FloatingToolbarColors {
        self.colors
    }
    /// Compose's `contentPadding`.
    pub fn content_padding(&self) -> PaddingValues {
        self.content_padding.clone()
    }
    /// Compose's `scrollBehavior`.
    pub fn scroll_behavior(&self) -> Option<Rc<dyn FloatingToolbarScrollBehavior>> {
        self.scroll_behavior.clone()
    }
    /// Compose's `shape`.
    pub fn shape(&self) -> Arc<dyn Shape> {
        self.shape.clone()
    }
    /// Compose's `floatingActionButtonPosition`.
    pub fn floating_action_button_position(&self) -> FloatingToolbarHorizontalFabPosition {
        self.floating_action_button_position
    }
    /// Compose's `animationSpec`.
    pub fn animation_spec(&self) -> Arc<dyn FiniteAnimationSpec<f32>> {
        self.animation_spec.clone()
    }
    /// Compose's `expandedShadowElevation`.
    pub fn expanded_shadow_elevation(&self) -> Dp {
        self.expanded_shadow_elevation
    }
    /// Compose's `collapsedShadowElevation`.
    pub fn collapsed_shadow_elevation(&self) -> Dp {
        self.collapsed_shadow_elevation
    }
    /// Compose's `content`.
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the horizontal floating toolbars with a FAB below (Compose's experimental
/// `LocalHorizontalFloatingToolbarWithFabOverride`): [`DefaultHorizontalFloatingToolbarWithFabOverride`] unless
/// provided.
pub static LocalHorizontalFloatingToolbarWithFabOverride: KompozitionLocal<
    Rc<dyn HorizontalFloatingToolbarWithFabOverride>,
> = KompozitionLocal::new(|| Rc::new(DefaultHorizontalFloatingToolbarWithFabOverride));

/// The vertical floating toolbar of an override (Compose's experimental `VerticalFloatingToolbarOverride`).
pub trait VerticalFloatingToolbarOverride {
    /// Composes the toolbar of `scope`'s arguments (Compose's
    /// `VerticalFloatingToolbarOverrideScope.VerticalFloatingToolbar()`).
    fn VerticalFloatingToolbar(&self, scope: &VerticalFloatingToolbarOverrideScope);
}

impl PartialEq for dyn VerticalFloatingToolbarOverride {
    /// The same override (by identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The arguments of a [`VerticalFloatingToolbar`], given to its override (Compose's experimental
/// `VerticalFloatingToolbarOverrideScope`).
#[derive(Clone)]
pub struct VerticalFloatingToolbarOverrideScope {
    is_expanded: bool,
    modifier: Modifier,
    colors: FloatingToolbarColors,
    content_padding: PaddingValues,
    scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    shape: Arc<dyn Shape>,
    leading_content: Option<Rc<dyn Fn()>>,
    trailing_content: Option<Rc<dyn Fn()>>,
    expanded_shadow_elevation: Dp,
    collapsed_shadow_elevation: Dp,
    content: Rc<dyn Fn()>,
}

impl VerticalFloatingToolbarOverrideScope {
    /// Compose's `isExpanded`.
    pub fn is_expanded(&self) -> bool {
        self.is_expanded
    }
    /// Compose's `modifier`.
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }
    /// Compose's `colors`.
    pub fn colors(&self) -> FloatingToolbarColors {
        self.colors
    }
    /// Compose's `contentPadding`.
    pub fn content_padding(&self) -> PaddingValues {
        self.content_padding.clone()
    }
    /// Compose's `scrollBehavior`.
    pub fn scroll_behavior(&self) -> Option<Rc<dyn FloatingToolbarScrollBehavior>> {
        self.scroll_behavior.clone()
    }
    /// Compose's `shape`.
    pub fn shape(&self) -> Arc<dyn Shape> {
        self.shape.clone()
    }
    /// Compose's `leadingContent`.
    pub fn leading_content(&self) -> Option<Rc<dyn Fn()>> {
        self.leading_content.clone()
    }
    /// Compose's `trailingContent`.
    pub fn trailing_content(&self) -> Option<Rc<dyn Fn()>> {
        self.trailing_content.clone()
    }
    /// Compose's `expandedShadowElevation`.
    pub fn expanded_shadow_elevation(&self) -> Dp {
        self.expanded_shadow_elevation
    }
    /// Compose's `collapsedShadowElevation`.
    pub fn collapsed_shadow_elevation(&self) -> Dp {
        self.collapsed_shadow_elevation
    }
    /// Compose's `content`.
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the vertical floating toolbars below (Compose's experimental
/// `LocalVerticalToolbarOverride`): [`DefaultVerticalFloatingToolbarOverride`] unless provided.
pub static LocalVerticalToolbarOverride: KompozitionLocal<Rc<dyn VerticalFloatingToolbarOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultVerticalFloatingToolbarOverride));

/// The vertical floating toolbar with a FAB of an override (Compose's experimental
/// `VerticalFloatingToolbarWithFabOverride`).
pub trait VerticalFloatingToolbarWithFabOverride {
    /// Composes the toolbar of `scope`'s arguments (Compose's
    /// `VerticalFloatingToolbarWithFabOverrideScope.VerticalFloatingToolbarWithFab()`).
    fn VerticalFloatingToolbarWithFab(&self, scope: &VerticalFloatingToolbarWithFabOverrideScope);
}

impl PartialEq for dyn VerticalFloatingToolbarWithFabOverride {
    /// The same override (by identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The arguments of a [`VerticalFloatingToolbarWithFab`], given to its override (Compose's experimental
/// `VerticalFloatingToolbarWithFabOverrideScope`).
#[derive(Clone)]
pub struct VerticalFloatingToolbarWithFabOverrideScope {
    is_expanded: bool,
    floating_action_button: Rc<dyn Fn()>,
    modifier: Modifier,
    colors: FloatingToolbarColors,
    content_padding: PaddingValues,
    scroll_behavior: Option<Rc<dyn FloatingToolbarScrollBehavior>>,
    shape: Arc<dyn Shape>,
    floating_action_button_position: FloatingToolbarVerticalFabPosition,
    animation_spec: Arc<dyn FiniteAnimationSpec<f32>>,
    expanded_shadow_elevation: Dp,
    collapsed_shadow_elevation: Dp,
    content: Rc<dyn Fn()>,
}

impl VerticalFloatingToolbarWithFabOverrideScope {
    /// Compose's `isExpanded`.
    pub fn is_expanded(&self) -> bool {
        self.is_expanded
    }
    /// Compose's `floatingActionButton`.
    pub fn floating_action_button(&self) -> Rc<dyn Fn()> {
        self.floating_action_button.clone()
    }
    /// Compose's `modifier`.
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }
    /// Compose's `colors`.
    pub fn colors(&self) -> FloatingToolbarColors {
        self.colors
    }
    /// Compose's `contentPadding`.
    pub fn content_padding(&self) -> PaddingValues {
        self.content_padding.clone()
    }
    /// Compose's `scrollBehavior`.
    pub fn scroll_behavior(&self) -> Option<Rc<dyn FloatingToolbarScrollBehavior>> {
        self.scroll_behavior.clone()
    }
    /// Compose's `shape`.
    pub fn shape(&self) -> Arc<dyn Shape> {
        self.shape.clone()
    }
    /// Compose's `floatingActionButtonPosition`.
    pub fn floating_action_button_position(&self) -> FloatingToolbarVerticalFabPosition {
        self.floating_action_button_position
    }
    /// Compose's `animationSpec`.
    pub fn animation_spec(&self) -> Arc<dyn FiniteAnimationSpec<f32>> {
        self.animation_spec.clone()
    }
    /// Compose's `expandedShadowElevation`.
    pub fn expanded_shadow_elevation(&self) -> Dp {
        self.expanded_shadow_elevation
    }
    /// Compose's `collapsedShadowElevation`.
    pub fn collapsed_shadow_elevation(&self) -> Dp {
        self.collapsed_shadow_elevation
    }
    /// Compose's `content`.
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the vertical floating toolbars with a FAB below (Compose's experimental
/// `LocalVerticalToolbarWithFabOverride`): [`DefaultVerticalFloatingToolbarWithFabOverride`] unless provided.
pub static LocalVerticalToolbarWithFabOverride: KompozitionLocal<Rc<dyn VerticalFloatingToolbarWithFabOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultVerticalFloatingToolbarWithFabOverride));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_toolbar_states_save_their_offsets() {
        let state = FloatingToolbarState(-80.0, -30.0, 120.0);
        let saver = floating_toolbar_state_saver();
        let restored = saver.restore(&saver.save(&state)).expect("restored");
        assert_eq!((restored.offset_limit(), restored.offset(), restored.content_offset()), (-80.0, -30.0, 120.0));
        assert!(*restored != *state, "a new state");
        // An offset set is kept between the limit and 0; a fraction of the way out is what settling goes by.
        restored.set_offset(-100.0);
        assert_eq!(restored.offset(), -80.0);
        assert_eq!(collapsed_fraction(&*restored), 1.0);
        restored.set_offset(-20.0);
        assert_eq!(collapsed_fraction(&*restored), 0.25);
        assert_eq!(collapsed_fraction(&*FloatingToolbarState(0.0, 0.0, 0.0)), 0.0);
    }
}
