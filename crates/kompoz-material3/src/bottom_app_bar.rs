//! Material's bottom app bars, as Compose's AppBar.kt has them: [`BottomAppBar`] (its actions and an optional
//! floating action button) and [`BottomAppBarWithContent`] (content of its own), the Expressive
//! [`FlexibleBottomAppBar`] (spread out, 64 dp), what they share (a Surface of a row, its height taken in by a
//! scroll behavior's offset), their scroll behavior ([`BottomAppBarScrollBehavior`]: exiting always — the bar goes
//! away as the content scrolls on and comes back as it scrolls back, settling after a fling or a drag of the bar),
//! its state ([`BottomAppBarState`], saved) and their defaults ([`BottomAppBarDefaults`]).

use crate::color_scheme::content_color_for;
use crate::internal::accessibility_service_state_provider::remember_accessibility_service_state;
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::surface::Surface;
use crate::tokens::{
    AppBarTokens, BottomAppBarTokens, DockedToolbarTokens, FabSecondaryContainerTokens, MotionSchemeKeyTokens,
};
use kompoz_animation::{
    AnimationScope, AnimationSpec, AnimationVector1D, DecayAnimationSpec, FloatDecaySpecOf, float_animation_state,
    remember_spline_based_decay,
};
use kompoz_coroutines::CoroutineScope;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Offset};
use kompoz_runtime::{
    MutableState, Saveable, SavedValue, Saver, State, kompozable, kompozable_block, list_saver, mutable_float_state_of,
    named, named_args, remember_keyed, remember_saveable_with,
};
use kompoz_ui::input::{FlingFuture, NestedScrollConnection, NestedScrollSource};
use kompoz_ui::unit::Velocity;
use kompoz_ui::{Alignment, Modifier};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- BottomAppBar, FlexibleBottomAppBar --------------------------------------------------------------------------

/// A bottom app bar (Compose's `BottomAppBar(actions, …)`, with or without a scroll behavior): navigation and key
/// actions at the bottom of small screens. `actions` (icon buttons, typically) in a row from the start, centered
/// across; the `floating_action_button`, if any, at the end, 12 dp from the top and 16 dp from the end (inside
/// `content_padding`). The rest as [`BottomAppBarWithContent`]'s: 80 dp tall (plus `window_insets`), of
/// `container_color` — `content_color` its content's (None: `content_color_for(container_color)`) — and
/// `tonal_elevation`; with a `scroll_behavior` (unless a touch exploration service is on) it goes away as the content
/// scrolls on and comes back as it scrolls back, and can be dragged.
///
/// Kotlin: `BottomAppBar(actions: @Composable RowScope.() -> Unit, modifier: Modifier = Modifier, floatingActionButton: @Composable (() -> Unit)? = null, containerColor: Color = BottomAppBarDefaults.containerColor, contentColor: Color = contentColorFor(containerColor), tonalElevation: Dp = BottomAppBarDefaults.ContainerElevation, contentPadding: PaddingValues = BottomAppBarDefaults.ContentPadding, windowInsets: WindowInsets = BottomAppBarDefaults.windowInsets, scrollBehavior: BottomAppBarScrollBehavior? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn BottomAppBar(
    actions: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] floating_action_button: Option<Rc<dyn Fn()>>,
    #[default(BottomAppBarDefaults.container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(BottomAppBarDefaults::ContainerElevation)] tonal_elevation: Dp,
    #[default(BottomAppBarDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(BottomAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(None)] scroll_behavior: Option<Rc<dyn BottomAppBarScrollBehavior>>,
) {
    let actions = Rc::new(actions);
    BottomAppBarWithContent(
        modifier = modifier,
        container_color = container_color,
        content_color = content_color,
        tonal_elevation = tonal_elevation,
        window_insets = window_insets,
        content_padding = content_padding,
        scroll_behavior = scroll_behavior,
        content = move || {
            let actions = actions.clone();
            Row(
                modifier = Modifier.weight(1.0),
                horizontal_arrangement = Arrangement::Start,
                vertical_alignment = Alignment::CenterVertically,
                content = move || actions(),
            );
            if let Some(floating_action_button) = floating_action_button.clone() {
                Box(
                    Modifier.fill_max_height().padding_sides(top = FABVerticalPadding, end = FABHorizontalPadding),
                    content_alignment = Alignment::TopStart,
                    content = move || floating_action_button(),
                );
            }
        },
    );
}

/// A bottom app bar of content of its own (Compose's `BottomAppBar(modifier, …, content)`, with or without a scroll
/// behavior): `content` in a row from the start, centered across, inside `content_padding` (4 dp at the start, the
/// top and the end: an icon button's room to be touched makes the rest), 80 dp tall plus `window_insets`, on a
/// [`Surface`] of `container_color` (`content_color` its content's — None: `content_color_for(container_color)` —,
/// `tonal_elevation`). With a `scroll_behavior` (unless a touch exploration service is on) it is laid out shorter by
/// the behavior's state's height offset (the state told the bar's height as the most it can go), so it goes away as
/// the content scrolls on and comes back as it scrolls back; it can be dragged too (unless the behavior is pinned),
/// settling as the behavior settles when let go.
///
/// Kotlin: `BottomAppBar(modifier: Modifier = Modifier, containerColor: Color = BottomAppBarDefaults.containerColor, contentColor: Color = contentColorFor(containerColor), tonalElevation: Dp = BottomAppBarDefaults.ContainerElevation, contentPadding: PaddingValues = BottomAppBarDefaults.ContentPadding, windowInsets: WindowInsets = BottomAppBarDefaults.windowInsets, scrollBehavior: BottomAppBarScrollBehavior? = null, content: @Composable RowScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn BottomAppBarWithContent(
    #[default(Modifier)] modifier: Modifier,
    #[default(BottomAppBarDefaults.container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(BottomAppBarDefaults::ContainerElevation)] tonal_elevation: Dp,
    #[default(BottomAppBarDefaults::ContentPadding())] content_padding: PaddingValues,
    #[default(BottomAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(None)] scroll_behavior: Option<Rc<dyn BottomAppBarScrollBehavior>>,
    content: impl Fn() + 'static,
) {
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    BottomAppBarLayout(
        BottomAppBarTokens::ContainerHeight,
        Arrangement::Start,
        modifier,
        container_color,
        content_color,
        tonal_elevation,
        content_padding,
        window_insets,
        scroll_behavior,
        Rc::new(content),
    );
}

/// The Expressive flexible bottom app bar (Compose's `FlexibleBottomAppBar`): `content` in a row put by
/// `horizontal_arrangement` (spread out, by default: the first at the start, the last at the end), centered across,
/// inside `content_padding` (16 dp at the sides), `expanded_height` tall (64 dp; one not finite or not above 0 is
/// the default) plus `window_insets`, on a [`Surface`] of `container_color` (`content_color` its content's — None:
/// `content_color_for(container_color)` —) without tonal elevation; a `scroll_behavior` takes it away and back as
/// [`BottomAppBarWithContent`]'s does.
///
/// Kotlin: `FlexibleBottomAppBar(modifier: Modifier = Modifier, containerColor: Color = BottomAppBarDefaults.containerColor, contentColor: Color = contentColorFor(containerColor), contentPadding: PaddingValues = BottomAppBarDefaults.FlexibleContentPadding, horizontalArrangement: Arrangement.Horizontal = BottomAppBarDefaults.FlexibleHorizontalArrangement, expandedHeight: Dp = BottomAppBarDefaults.FlexibleBottomAppBarHeight, windowInsets: WindowInsets = BottomAppBarDefaults.windowInsets, scrollBehavior: BottomAppBarScrollBehavior? = null, content: @Composable RowScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn FlexibleBottomAppBar(
    #[default(Modifier)] modifier: Modifier,
    #[default(BottomAppBarDefaults.container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(BottomAppBarDefaults::FlexibleContentPadding())] content_padding: PaddingValues,
    #[default(BottomAppBarDefaults::FlexibleHorizontalArrangement)] horizontal_arrangement: HorizontalArrangement,
    #[default(BottomAppBarDefaults::FlexibleBottomAppBarHeight)] expanded_height: Dp,
    #[default(BottomAppBarDefaults.window_insets())] window_insets: WindowInsets,
    #[default(None)] scroll_behavior: Option<Rc<dyn BottomAppBarScrollBehavior>>,
    content: impl Fn() + 'static,
) {
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    BottomAppBarLayout(
        if expanded_height.is_finite() && expanded_height.is_specified() && expanded_height > Dp(0.0) {
            expanded_height
        } else {
            BottomAppBarDefaults::FlexibleBottomAppBarHeight
        },
        horizontal_arrangement,
        modifier,
        container_color,
        content_color,
        AppBarTokens::ContainerElevation,
        content_padding,
        window_insets,
        scroll_behavior,
        Rc::new(content),
    );
}

/// What the bottom app bars share (Compose's private `BottomAppBarLayout`): a [`Surface`] (the bar's token shape,
/// square) of a row `container_height` tall inside `window_insets` and `content_padding`, laid out shorter by the
/// active scroll behavior's height offset (and telling its state the most it can go: the whole height), draggable
/// when the behavior isn't pinned.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn BottomAppBarLayout(
    container_height: Dp,
    horizontal_arrangement: HorizontalArrangement,
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
    tonal_elevation: Dp,
    content_padding: PaddingValues,
    window_insets: WindowInsets,
    scroll_behavior: Option<Rc<dyn BottomAppBarScrollBehavior>>,
    content: Rc<dyn Fn()>,
) {
    let touch_exploration_service_enabled = remember_touch_exploration_service().get();
    let active_scroll_behavior = if !touch_exploration_service_enabled { scroll_behavior } else { None };
    // Set up support for resizing the bottom app bar when vertically dragging the bar itself.
    let app_bar_drag_modifier = match &active_scroll_behavior {
        Some(behavior) if !behavior.is_pinned() => {
            let (dragged, settled) = (behavior.clone(), behavior.clone());
            named_args!(Modifier.draggable(
                orientation = Orientation::Vertical,
                state = remember_draggable_state(move |delta| {
                    let state = dragged.state();
                    state.set_height_offset(state.height_offset() - delta);
                }),
                on_drag_stopped = Some(Rc::new(move |_: CoroutineScope, velocity: f32| {
                    let behavior = settled.clone();
                    Box::pin(async move {
                        settle_app_bar_bottom(
                            behavior.state(),
                            velocity,
                            behavior.fling_animation_spec(),
                            behavior.snap_animation_spec(),
                        )
                        .await;
                    }) as LocalFuture<'static, ()>
                }) as OnDragStopped),
            ))
        }
        _ => Modifier,
    };

    // Compose a Surface with a Row content.
    // The height of the app bar is determined by subtracting the bar's height offset from the app bar's defined
    // constant height value (i.e. the ContainerHeight token).
    let measured_behavior = active_scroll_behavior.clone();
    Surface(
        color = container_color,
        content_color = Some(content_color),
        tonal_elevation = tonal_elevation,
        // (Compose's TODO(b/209583788): consider adding a shape parameter if updated design guidance allows)
        shape = BottomAppBarTokens::ContainerShape.value(),
        modifier = modifier
            .layout(move |scope, measurable, constraints| {
                let placeable = measurable.measure(constraints);

                // Sets the app bar's height offset to collapse the entire bar's height when content is scrolled.
                let state = measured_behavior.as_ref().map(|b| b.state());
                if let Some(state) = &state {
                    state.set_height_offset_limit(-(placeable.height() as f32));
                }

                let height = (placeable.height() as f32 + state.as_ref().map_or(0.0, |s| s.height_offset())).max(0.0);
                scope.layout(placeable.width(), height.round() as i32, move |p| p.place(&placeable, 0, 0))
            })
            .then(app_bar_drag_modifier),
        content = move || {
            Row(
                Modifier
                    .fill_max_width()
                    .window_insets_padding(window_insets)
                    .height(container_height)
                    .padding_values(content_padding),
                horizontal_arrangement = horizontal_arrangement,
                vertical_alignment = Alignment::CenterVertically,
                content = move || content(),
            );
        },
    );
}

/// Whether a touch exploration service is on (Compose's private `rememberTouchExplorationService`): a bar's scroll
/// behavior is then left out (the desktop has none: false).
#[kompozable]
fn remember_touch_exploration_service() -> State<bool> {
    remember_accessibility_service_state(
        listen_to_touch_exploration_state = true,
        listen_to_switch_access_state = false,
        listen_to_voice_access_state = false,
    )
}

// ---- BottomAppBarScrollBehavior ----------------------------------------------------------------------------------

/// How a bottom app bar behaves as the content scrolls (Compose's `BottomAppBarScrollBehavior`): the
/// [`BottomAppBarState`] it moves, whether the bar stays (`is_pinned`: it then takes no drags either), how it
/// settles after a fling or a drag (its specs), and the nested scroll connection to put on the scrolled content's
/// parent (`Modifier.nested_scroll(behavior.nested_scroll_connection())`). Made by
/// [`BottomAppBarDefaults::exit_always_scroll_behavior`]; shared as an `Rc<dyn BottomAppBarScrollBehavior>`, equal
/// only to itself.
pub trait BottomAppBarScrollBehavior {
    /// The state the bar is laid out by, which this behavior moves (Compose's `state`).
    fn state(&self) -> Rc<dyn BottomAppBarState>;

    /// Whether the bar stays where it is as the content scrolls (Compose's `isPinned`).
    fn is_pinned(&self) -> bool;

    /// What snaps the bar to shown or gone once a fling or a drag leaves it in between (Compose's
    /// `snapAnimationSpec`): none, it stays.
    fn snap_animation_spec(&self) -> Option<Arc<dyn AnimationSpec<f32>>>;

    /// What carries a fling's speed on into the bar's going or coming back (Compose's `flingAnimationSpec`).
    fn fling_animation_spec(&self) -> Option<Arc<dyn DecayAnimationSpec<f32>>>;

    /// The connection to give the scrolled content's parent (Compose's `nestedScrollConnection`): what it hears of
    /// the content's scrolls and flings moves the state.
    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection>;
}

impl PartialEq for dyn BottomAppBarScrollBehavior {
    /// The same behavior (Kotlin's `==` of these objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl fmt::Debug for dyn BottomAppBarScrollBehavior {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BottomAppBarScrollBehavior(state={:?}, isPinned={})", &*self.state(), self.is_pinned())
    }
}

// ---- BottomAppBarDefaults ----------------------------------------------------------------------------------------

/// The bottom app bars' defaults (Compose's `BottomAppBarDefaults`): their colors, elevation, paddings, insets, the
/// flexible bar's height and arrangements, the scroll behavior. What Compose reads in composition is a method of the
/// value (`BottomAppBarDefaults.container_color()`); its constants are associated consts or functions
/// (`BottomAppBarDefaults::ContainerElevation`, `BottomAppBarDefaults::ContentPadding()`).
#[derive(Clone, Copy, Debug)]
pub struct BottomAppBarDefaults;

#[named]
impl BottomAppBarDefaults {
    /// A bottom app bar's elevation: 0 dp (Compose's `ContainerElevation`).
    pub const ContainerElevation: Dp = Dp(0.0);

    /// The flexible bottom app bar's height: 64 dp (Compose's `FlexibleBottomAppBarHeight`).
    pub const FlexibleBottomAppBarHeight: Dp = DockedToolbarTokens::ContainerHeight;

    /// What spaces a flexible bottom app bar's content: the first at the start, the last at the end, the room
    /// between them shared (Compose's `FlexibleHorizontalArrangement`, `Arrangement.SpaceBetween`).
    pub const FlexibleHorizontalArrangement: HorizontalArrangement = Arrangement::SpaceBetween;

    /// A bottom app bar's container color: the scheme's surface container (Compose's `containerColor`).
    pub fn container_color(&self) -> Color {
        BottomAppBarTokens::ContainerColor.value()
    }

    /// A bottom app bar's content padding: 4 dp at the start, the top and the end (Compose's `ContentPadding`; 16
    /// and 12 dp less an icon button's room to be touched).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::new(
            BottomAppBarHorizontalPadding,
            BottomAppBarVerticalPadding,
            BottomAppBarHorizontalPadding,
            Dp(0.0),
        )
    }

    /// The insets a bottom app bar keeps clear of (and consumes) by default (Compose's `windowInsets`): the system
    /// bars' at the sides and the bottom.
    #[track_caller]
    pub fn window_insets(&self) -> WindowInsets {
        system_bars_for_visual_components().only(WindowInsetsSides::Horizontal + WindowInsetsSides::Bottom)
    }

    /// The color of a bottom app bar's floating action button: the scheme's secondary container (Compose's
    /// `bottomAppBarFabColor`).
    pub fn bottom_app_bar_fab_color(&self) -> Color {
        FabSecondaryContainerTokens::ContainerColor.value()
    }

    /// A flexible bottom app bar's content padding: 16 dp at the start and the end (Compose's
    /// `FlexibleContentPadding`).
    pub fn FlexibleContentPadding() -> PaddingValues {
        PaddingValues::new(
            DockedToolbarTokens::ContainerLeadingSpace,
            Dp(0.0),
            DockedToolbarTokens::ContainerTrailingSpace,
            Dp(0.0),
        )
    }

    /// What spaces a flexible bottom app bar's content fixed: 32 dp between them, centered (Compose's
    /// `FlexibleFixedHorizontalArrangement`).
    pub fn FlexibleFixedHorizontalArrangement() -> HorizontalArrangement {
        Arrangement::spaced_by_aligned(DockedToolbarTokens::ContainerMaxSpacing, Alignment::CenterHorizontally)
    }

    /// A bottom app bar's behavior exiting always (Compose's `exitAlwaysScrollBehavior`): the bar goes away as the
    /// content scrolls on and comes back as it scrolls back, while `can_scroll` says so; after a fling it goes on
    /// along `fling_animation_spec`, then snaps by `snap_animation_spec` to shown or gone. Remembered while `state`,
    /// `can_scroll` and the specs stay. (As Compose's TODO says, it may leave the bar out of an assistive
    /// technology's reach: the bars leave a behavior out while a touch exploration service is on.)
    ///
    /// Kotlin: `exitAlwaysScrollBehavior(state: BottomAppBarState = rememberBottomAppBarState(), canScroll: () -> Boolean = { true }, snapAnimationSpec: AnimationSpec<Float>? = MotionSchemeKeyTokens.FastSpatial.value(), flingAnimationSpec: DecayAnimationSpec<Float>? = rememberSplineBasedDecay()): BottomAppBarScrollBehavior`
    #[track_caller]
    pub fn exit_always_scroll_behavior(
        &self,
        #[default(remember_bottom_app_bar_state())] state: Rc<dyn BottomAppBarState>,
        #[default(|| true)]
        #[memoized]
        can_scroll: impl Fn() -> bool + 'static,
        #[default(Some(MotionSchemeKeyTokens::FastSpatial.value::<f32>() as Arc<dyn AnimationSpec<f32>>))]
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        #[default(Some(Arc::new(remember_spline_based_decay::<f32>()) as Arc<dyn DecayAnimationSpec<f32>>))]
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    ) -> Rc<dyn BottomAppBarScrollBehavior> {
        let can_scroll_id = can_scroll.id();
        let can_scroll: Rc<dyn Fn() -> bool> = Rc::new(can_scroll.into_value());
        kompozable_block!({
            let keys = (
                StateKey(state.clone()),
                can_scroll_id,
                snap_animation_spec.clone(),
                DecayKey(fling_animation_spec.clone()),
            );
            remember_keyed(keys, move || {
                Rc::new(ExitAlwaysScrollBehavior::new(state, snap_animation_spec, fling_animation_spec, can_scroll))
                    as Rc<dyn BottomAppBarScrollBehavior>
            })
        })
    }
}

/// A state compared by its instance (Compose's `remember(state, …)` key).
struct StateKey(Rc<dyn BottomAppBarState>);

impl PartialEq for StateKey {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(Rc::as_ptr(&self.0), Rc::as_ptr(&other.0))
    }
}

/// A decay spec compared as a `remember` key: by its instance, or the float decay spec `remember_spline_based_decay`
/// remembers inside its adapter (made again each call).
struct DecayKey(Option<Arc<dyn DecayAnimationSpec<f32>>>);

impl PartialEq for DecayKey {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
                    || match (
                        a.as_any().downcast_ref::<FloatDecaySpecOf<f32>>(),
                        b.as_any().downcast_ref::<FloatDecaySpecOf<f32>>(),
                    ) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a.float_decay_spec(), b.float_decay_spec()),
                        _ => false,
                    }
            }
            _ => false,
        }
    }
}

// ---- BottomAppBarState -------------------------------------------------------------------------------------------

/// A [`BottomAppBarState`] remembered and saved (Compose's `rememberBottomAppBarState`): of the given offsets at
/// first.
///
/// Kotlin: `rememberBottomAppBarState(initialHeightOffsetLimit: Float = -Float.MAX_VALUE, initialHeightOffset: Float = 0f, initialContentOffset: Float = 0f): BottomAppBarState`
#[kompozable]
pub fn remember_bottom_app_bar_state(
    #[default(-f32::MAX)] initial_height_offset_limit: f32,
    #[default(0.0)] initial_height_offset: f32,
    #[default(0.0)] initial_content_offset: f32,
) -> Rc<dyn BottomAppBarState> {
    remember_saveable_with(&<dyn BottomAppBarState>::Saver(), move || {
        BottomAppBarState(initial_height_offset_limit, initial_height_offset, initial_content_offset)
    })
}

/// The state of a bottom app bar's scroll behavior (Compose's interface `BottomAppBarState`): how far the bar went
/// (`height_offset`, from 0 down to `height_offset_limit`, negative: the bar's height), how far the content scrolled
/// (`content_offset`), and the fraction gone. Its properties are read as states (what reads them is told as they
/// change). Made by [`BottomAppBarState()`](fn@BottomAppBarState), in composition by [`remember_bottom_app_bar_state`];
/// shared as an `Rc<dyn BottomAppBarState>`.
pub trait BottomAppBarState {
    /// The most the bar can go, negative: what its height offset stops at (Compose's `heightOffsetLimit`; the bar
    /// sets it to its height as it is laid out).
    fn height_offset_limit(&self) -> f32;

    /// Sets [`height_offset_limit`](BottomAppBarState::height_offset_limit).
    fn set_height_offset_limit(&self, value: f32);

    /// How far the bar went, from 0 (all shown) down to the limit (Compose's `heightOffset`).
    fn height_offset(&self) -> f32;

    /// Sets [`height_offset`](BottomAppBarState::height_offset), kept between the limit and 0.
    fn set_height_offset(&self, value: f32);

    /// How far the content scrolled, in all (Compose's `contentOffset`): the behaviors use it to know whether the
    /// content is at its start.
    fn content_offset(&self) -> f32;

    /// Sets [`content_offset`](BottomAppBarState::content_offset).
    fn set_content_offset(&self, value: f32);

    /// The fraction gone, the height offset over the limit (Compose's `collapsedFraction`): 0 shown, 1 gone (0 with a
    /// limit of 0).
    fn collapsed_fraction(&self) -> f32;
}

impl dyn BottomAppBarState {
    /// Saves a state's limit and offsets, restored as a state of them (Compose's `BottomAppBarState.Saver`).
    #[allow(non_snake_case)]
    pub fn Saver() -> Saver<Rc<dyn BottomAppBarState>> {
        list_saver(
            |it: &Rc<dyn BottomAppBarState>| {
                vec![it.height_offset_limit().save(), it.height_offset().save(), it.content_offset().save()]
            },
            |saved: &[SavedValue]| {
                Some(BottomAppBarState(
                    f32::restore(saved.first()?)?,
                    f32::restore(saved.get(1)?)?,
                    f32::restore(saved.get(2)?)?,
                ))
            },
        )
    }
}

impl fmt::Debug for dyn BottomAppBarState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BottomAppBarState(heightOffsetLimit={}, heightOffset={}, contentOffset={})",
            self.height_offset_limit(),
            self.height_offset(),
            self.content_offset()
        )
    }
}

/// A bottom app bar state of the given limit and offsets (Compose's function `BottomAppBarState(…)`): in
/// composition, rather [`remember_bottom_app_bar_state`].
///
/// Kotlin: `BottomAppBarState(initialHeightOffsetLimit: Float, initialHeightOffset: Float, initialContentOffset: Float): BottomAppBarState`
pub fn BottomAppBarState(
    initial_height_offset_limit: f32,
    initial_height_offset: f32,
    initial_content_offset: f32,
) -> Rc<dyn BottomAppBarState> {
    Rc::new(BottomAppBarStateImpl::new(initial_height_offset_limit, initial_height_offset, initial_content_offset))
}

/// Compose's private `BottomAppBarStateImpl`: three float states, the height offset kept between the limit and 0.
struct BottomAppBarStateImpl {
    height_offset_limit: MutableState<f32>,
    height_offset: MutableState<f32>,
    content_offset: MutableState<f32>,
}

impl BottomAppBarStateImpl {
    fn new(initial_height_offset_limit: f32, initial_height_offset: f32, initial_content_offset: f32) -> Self {
        BottomAppBarStateImpl {
            height_offset_limit: mutable_float_state_of(initial_height_offset_limit),
            height_offset: mutable_float_state_of(initial_height_offset),
            content_offset: mutable_float_state_of(initial_content_offset),
        }
    }
}

impl BottomAppBarState for BottomAppBarStateImpl {
    fn height_offset_limit(&self) -> f32 {
        self.height_offset_limit.get()
    }
    fn set_height_offset_limit(&self, value: f32) {
        self.height_offset_limit.set(value);
    }
    fn height_offset(&self) -> f32 {
        self.height_offset.get()
    }
    fn set_height_offset(&self, value: f32) {
        // (Kotlin's `coerceIn(minimumValue = heightOffsetLimit, maximumValue = 0f)`: the maximum wins a limit above it)
        self.height_offset.set(value.max(self.height_offset_limit()).min(0.0));
    }
    fn content_offset(&self) -> f32 {
        self.content_offset.get()
    }
    fn set_content_offset(&self, value: f32) {
        self.content_offset.set(value);
    }
    fn collapsed_fraction(&self) -> f32 {
        let limit = self.height_offset_limit();
        if limit != 0.0 { self.height_offset() / limit } else { 0.0 }
    }
}

// ---- ExitAlwaysScrollBehavior ------------------------------------------------------------------------------------

/// The exiting-always behavior (Compose's private `ExitAlwaysScrollBehavior`): what the content scrolls moves the
/// bar the same way (away as it scrolls on, back as it scrolls back), while `can_scroll` says so; settles after a
/// fling. Never pinned.
struct ExitAlwaysScrollBehavior {
    state: Rc<dyn BottomAppBarState>,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    nested_scroll_connection: Rc<dyn NestedScrollConnection>,
}

impl ExitAlwaysScrollBehavior {
    fn new(
        state: Rc<dyn BottomAppBarState>,
        snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
        fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
        can_scroll: Rc<dyn Fn() -> bool>,
    ) -> Self {
        let nested_scroll_connection = Rc::new(ExitAlwaysConnection {
            state: state.clone(),
            snap_animation_spec: snap_animation_spec.clone(),
            fling_animation_spec: fling_animation_spec.clone(),
            can_scroll,
        });
        ExitAlwaysScrollBehavior { state, snap_animation_spec, fling_animation_spec, nested_scroll_connection }
    }
}

impl BottomAppBarScrollBehavior for ExitAlwaysScrollBehavior {
    fn state(&self) -> Rc<dyn BottomAppBarState> {
        self.state.clone()
    }
    fn is_pinned(&self) -> bool {
        false
    }
    fn snap_animation_spec(&self) -> Option<Arc<dyn AnimationSpec<f32>>> {
        self.snap_animation_spec.clone()
    }
    fn fling_animation_spec(&self) -> Option<Arc<dyn DecayAnimationSpec<f32>>> {
        self.fling_animation_spec.clone()
    }
    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection> {
        self.nested_scroll_connection.clone()
    }
}

/// The behavior's nested scroll connection (Compose's anonymous `object : NestedScrollConnection` in it).
struct ExitAlwaysConnection {
    state: Rc<dyn BottomAppBarState>,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    can_scroll: Rc<dyn Fn() -> bool>,
}

impl NestedScrollConnection for ExitAlwaysConnection {
    fn on_post_scroll(&self, consumed: Offset, _available: Offset, _source: NestedScrollSource) -> Offset {
        if !(self.can_scroll)() {
            return Offset::Zero;
        }
        let state = &self.state;
        state.set_content_offset(state.content_offset() + consumed.y);
        state.set_height_offset(state.height_offset() + consumed.y);
        Offset::Zero
    }

    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            let state = &self.state;
            if available.y > 0.0
                && (state.height_offset() == 0.0 || state.height_offset() == state.height_offset_limit())
            {
                // Reset the total content offset to zero when scrolling all the way down. This will eliminate some
                // float precision inaccuracies.
                state.set_content_offset(0.0);
            }
            // (the default's: nothing consumed)
            let super_consumed = Velocity::Zero;
            super_consumed
                + settle_app_bar_bottom(
                    state.clone(),
                    available.y,
                    self.fling_animation_spec.clone(),
                    self.snap_animation_spec.clone(),
                )
                .await
        })
    }
}

/// Settles a bottom bar left in between (Compose's private `settleAppBarBottom`): a fling's `velocity` carried on
/// along `fling_animation_spec` (stopping where the bar takes no more of it), then a snap by `snap_animation_spec` to
/// shown (less than half gone) or gone. Nothing when it is (all but) shown or gone. The velocity left.
async fn settle_app_bar_bottom(
    state: Rc<dyn BottomAppBarState>,
    velocity: f32,
    fling_animation_spec: Option<Arc<dyn DecayAnimationSpec<f32>>>,
    snap_animation_spec: Option<Arc<dyn AnimationSpec<f32>>>,
) -> Velocity {
    // Check if the app bar is completely collapsed/expanded. If so, no need to settle the app bar, and just return
    // Zero Velocity.
    // Note that we don't check for 0f due to float precision with the collapsedFraction calculation.
    if state.collapsed_fraction() < 0.01 || state.collapsed_fraction() == 1.0 {
        return Velocity::Zero;
    }
    let remaining_velocity = Rc::new(Cell::new(velocity));
    // In case there is an initial velocity that was left after a previous user fling, animate to continue the motion
    // to expand or collapse the app bar.
    if let Some(fling_animation_spec) = fling_animation_spec
        && velocity.abs() > 1.0
    {
        let last_value = Cell::new(0.0f32);
        let (state, remaining) = (state.clone(), remaining_velocity.clone());
        let block = move |scope: &AnimationScope<f32, AnimationVector1D>| {
            let delta = scope.value() - last_value.get();
            let initial_height_offset = state.height_offset();
            state.set_height_offset(initial_height_offset + delta);
            let consumed = (initial_height_offset - state.height_offset()).abs();
            last_value.set(scope.value());
            remaining.set(scope.velocity());
            // avoid rounding errors and stop if anything is unconsumed
            if (delta - consumed).abs() > 0.5 {
                scope.cancel_animation();
            }
        };
        named_args!(
            float_animation_state(0.0, initial_velocity = velocity)
                .animate_decay(fling_animation_spec, block = Box::new(block))
        )
        .await;
    }
    // Snap if animation specs were provided.
    if let Some(snap_animation_spec) = snap_animation_spec
        && state.height_offset() < 0.0
        && state.height_offset() > state.height_offset_limit()
    {
        let target = if state.collapsed_fraction() < 0.5 { 0.0 } else { state.height_offset_limit() };
        let st = state.clone();
        let block = move |scope: &AnimationScope<f32, AnimationVector1D>| st.set_height_offset(scope.value());
        named_args!(float_animation_state(state.height_offset()).animate_to(
            target,
            animation_spec = snap_animation_spec,
            block = Box::new(block)
        ))
        .await;
    }

    Velocity::new(0.0, remaining_velocity.get())
}

// Padding minus IconButton's min touch target expansion
const BottomAppBarHorizontalPadding: Dp = Dp(16.0 - 12.0);
pub(crate) const BottomAppBarVerticalPadding: Dp = Dp(16.0 - 12.0);

// Padding minus content padding
const FABHorizontalPadding: Dp = Dp(16.0 - BottomAppBarHorizontalPadding.0);
const FABVerticalPadding: Dp = Dp(12.0 - BottomAppBarVerticalPadding.0);
