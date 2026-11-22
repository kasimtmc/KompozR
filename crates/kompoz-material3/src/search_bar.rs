//! Search bars, as Compose's SearchBar.kt: a bar of an input field ([`SearchBar`], [`AppBarWithSearch`],
//! deprecated [`TopSearchBar`]) which, through its [`SearchBarState`], expands into a view of results over the
//! whole window ([`ExpandedFullScreenSearchBar`], [`ExpandedFullScreenContainedSearchBar`]) or a drop-down at its
//! place ([`ExpandedDockedSearchBar`], [`ExpandedDockedSearchBarWithGap`]); the older bars that expand themselves
//! ([`SearchBarWithContent`], [`DockedSearchBar`], deprecated [`StringSearchBar`], [`StringDockedSearchBar`]); the
//! states ([`SearchBarState`], [`remember_search_bar_state`], [`remember_contained_search_bar_state`],
//! [`remember_with_gap_search_bar_state`]) and the scroll behavior that slides a bar away with the content
//! ([`SearchBarScrollBehavior`]). The defaults, colors and input fields are in search_bar_defaults.rs.
//!
//! Compose's overloads are composables of their own here, named by what tells them apart: [`SearchBar`] (Kotlin's
//! `SearchBar(state, inputField, …)`), [`SearchBarWithContent`] (`SearchBar(inputField, expanded,
//! onExpandedChange, …, content)`), [`StringSearchBar`] (the deprecated `SearchBar(query: String, …)`) and
//! [`StringDockedSearchBar`] (the deprecated `DockedSearchBar(query: String, …)`).

// (the deprecated composables' builders call them)
#![allow(deprecated)]

use crate::color_scheme::content_color_for;
use crate::content_color::LocalContentColor;
use crate::divider::HorizontalDivider;
use crate::internal::back_handler::{BackEventCompat, BackHandler, PredictiveBack, PredictiveBackHandler, SwipeEdge};
use crate::internal::basic_edge_to_edge_dialog::{
    BackEventProgress, BasicEdgeToEdgeDialog, PredictiveBackState, SwipeEdge as ProgressSwipeEdge,
};
use crate::internal::mutable_window_insets::MutableWindowInsets;
use crate::internal::shape_util::ClipShape;
use crate::search_bar_defaults::{
    AppBarWithSearchColors, InputFieldHeight, SearchBarColors, SearchBarDefaults, get_window_container_height,
};
use crate::surface::{ColorProducerSurface, Surface};
use crate::tokens::{MotionSchemeKeyTokens, MotionTokens};
use kompoz_animation::{
    Animatable, AnimatedVisibility, AnimatedVisibilityScope, AnimationSpec, AnimationState, AnimationVector1D,
    CubicBezierEasing, DecayAnimationSpec, EnterTransition, ExitTransition, FiniteAnimationSpec, MutatorMutex,
    SnapSpec, SpringSpec, TweenSpec, animate_color_as_state, expand_vertically, fade_in, fade_out,
    float_animation_state, shrink_vertically, slide_in, slide_out, snap,
};
use kompoz_coroutines::{CoroutineScope, Flow, LaunchedEffect, coroutine_scope, remember_coroutine_scope};
use kompoz_foundation::gestures::{DraggableState, LocalFuture, Orientation};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{
    CircleShape, Color, ColorProducer, CornerRadius, Density, Dp, IntOffset, IntRect, IntSize, LayoutDirection, Offset,
    Outline, Path, PathDirection, RectangleShape, RoundRect, Shape, Size,
};
use kompoz_runtime::{
    DerivedState, KompozitionLocalProvider, MutableState, SavedValue, Saver, SharedState, derived_state_of, kompozable,
    mutable_state_of, named, named_args, remember, remember_keyed, remember_saveable_keyed_with,
};
use kompoz_ui::focus::{FocusDirection, FocusRequester};
use kompoz_ui::input::{FlingFuture, NestedScrollConnection, NestedScrollSource};
use kompoz_ui::layout::{
    GraphicsLayerScope, Layout, LayoutCoordinates, Measurable, MeasureResult, MeasureScope, MultiContentLayout,
    MultiContentMeasurePolicy, measure_policy,
};
use kompoz_ui::platform::{LocalDensity, LocalSoftwareKeyboardController};
use kompoz_ui::platform_text_input::{InterceptPlatformTextInput, PlatformTextInputInterceptor, await_cancellation};
use kompoz_ui::unit::Velocity;
use kompoz_ui::window::{DialogProperties, Popup, PopupPositionProvider, PopupProperties};
use kompoz_ui::{Alignment, Constraints, Modifier};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

/// An input field slot of a search bar (Compose's `inputField: @Composable () -> Unit`).
pub type SearchBarSlot = Rc<dyn Fn()>;

// ---------------------------------------------------------------------------------------------------------------
// The bars of a state

/// A search bar, collapsed, of `state` (Compose's `SearchBar(state, inputField, …)`): `input_field` (typically a
/// [`SearchBarDefaults`]`.InputField(…)` of the same state) on a [`Surface`] of `shape`, `colors`' container color,
/// `tonal_elevation` and `shadow_elevation`; where it is is the state's [`collapsed_coords`]
/// (SearchBarState::collapsed_coords), which its expanded forms ([`ExpandedFullScreenSearchBar`],
/// [`ExpandedDockedSearchBar`], …) grow from. A text field in it doesn't show the software keyboard (the
/// expanded view's field takes the typing).
///
/// Kotlin: `SearchBar(state: SearchBarState, inputField: @Composable () -> Unit, modifier: Modifier = Modifier, shape: Shape = SearchBarDefaults.inputFieldShape, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SearchBar(
    state: SearchBarState,
    input_field: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SearchBarDefaults::input_field_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
) {
    let input_field: SearchBarSlot = Rc::new(input_field);
    DisableSoftKeyboard(move || {
        let state = state.clone();
        let input_field = input_field.clone();
        Surface(
            modifier = modifier.clone().on_globally_positioned(move |it: &LayoutCoordinates| {
                state.set_collapsed_coords(Some(it.clone()));
            }),
            shape = shape.clone(),
            color = colors.container_color,
            content_color = Some(content_color_for(colors.container_color)),
            tonal_elevation = tonal_elevation,
            shadow_elevation = shadow_elevation,
            content = move || input_field(),
        );
    });
}

/// Compose's deprecated `TopSearchBar`, renamed [`AppBarWithSearch`]: an app bar of a search bar alone, its
/// container transparent (the search bar's `colors`, no scrolled colors).
///
/// Kotlin: `TopSearchBar(state: SearchBarState, inputField: @Composable () -> Unit, modifier: Modifier = Modifier, shape: Shape = SearchBarDefaults.inputFieldShape, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, windowInsets: WindowInsets = SearchBarDefaults.windowInsets, scrollBehavior: SearchBarScrollBehavior? = null)`
#[deprecated(note = "Renamed to `AppBarWithSearch`")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn TopSearchBar(
    state: SearchBarState,
    input_field: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SearchBarDefaults::input_field_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(SearchBarDefaults::window_insets())] window_insets: WindowInsets,
    #[default(None)] scroll_behavior: Option<Rc<dyn SearchBarScrollBehavior>>,
) {
    AppBarWithSearch(
        state,
        input_field,
        modifier = modifier,
        shape = shape,
        colors = SearchBarDefaults.app_bar_with_search_colors(
            search_bar_colors = colors,
            scrolled_search_bar_container_color = Color::Unspecified,
            app_bar_container_color = Color::Transparent,
            scrolled_app_bar_container_color = Color::Unspecified,
        ),
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        window_insets = window_insets,
        scroll_behavior = scroll_behavior,
    );
}

/// An app bar of a search bar (Compose's `AppBarWithSearch`): a [`SearchBar`] of `state` and `input_field` filling
/// the width between `navigation_icon` (None: none) and `actions` (None: none, else in a Row at the end), 360 to
/// 720 dp wide, centred, padded 8 dp at the sides and 4 dp above and below; `navigation_icon` and `actions` 4 dp
/// in from the sides, in `colors`' icon colors. On a surface of `colors`' app bar color (animated to its scrolled
/// color while content is scrolled under it, as `scroll_behavior` says), padded by `content_padding` and
/// `window_insets`. Its container transparent, the search bar takes the elevations; else the bar does. Through
/// `scroll_behavior` (None: it stays) it slides away with the content and back. The search bar fades out while
/// its full-screen form is open (not to show twice).
///
/// Kotlin: `AppBarWithSearch(state: SearchBarState, inputField: @Composable () -> Unit, modifier: Modifier = Modifier, navigationIcon: @Composable (() -> Unit)? = null, actions: @Composable (RowScope.() -> Unit)? = null, shape: Shape = SearchBarDefaults.inputFieldShape, colors: AppBarWithSearchColors = SearchBarDefaults.appBarWithSearchColors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, contentPadding: PaddingValues = SearchBarDefaults.AppBarContentPadding, windowInsets: WindowInsets = SearchBarDefaults.windowInsets, scrollBehavior: SearchBarScrollBehavior? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn AppBarWithSearch(
    state: SearchBarState,
    input_field: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] navigation_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] actions: Option<Rc<dyn Fn()>>,
    #[default(SearchBarDefaults::input_field_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.app_bar_with_search_colors())] colors: AppBarWithSearchColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(SearchBarDefaults::AppBarContentPadding())] content_padding: PaddingValues,
    #[default(SearchBarDefaults::window_insets())] window_insets: WindowInsets,
    #[default(None)] scroll_behavior: Option<Rc<dyn SearchBarScrollBehavior>>,
) {
    // TODO Load the motionScheme tokens from the component tokens file
    let animation_spec = MotionSchemeKeyTokens::DefaultEffects;

    let is_container_transparent = remember_keyed(colors, || colors.app_bar_container_color == Color::Transparent);

    let behavior_key = scroll_behavior.as_ref().map(|b| Rc::as_ptr(b) as *const () as usize);
    let target_colors: DerivedState<(Color, Color)> = {
        let scroll_behavior = scroll_behavior.clone();
        remember_keyed((colors, behavior_key), move || {
            derived_state_of(move || {
                let overlapping_fraction = scroll_behavior.as_ref().map_or(0.0, |b| overlapped_fraction(&**b));
                let color_transition_fraction = if overlapping_fraction > 0.01 { 1.0 } else { 0.0 };
                (
                    colors.app_bar_container_color_at(color_transition_fraction),
                    colors.search_bar_container_color_at(color_transition_fraction),
                )
            })
        })
    };
    let (target_app_bar_container_color, target_search_bar_container_color) = target_colors.get();

    let app_bar_container_color =
        animate_color_as_state(target_app_bar_container_color, animation_spec = animation_spec.value::<Color>());

    let search_bar_colors = remember_keyed(target_search_bar_container_color, || {
        named_args!(colors.search_bar_colors.copy(container_color = target_search_bar_container_color))
    });

    let bar_modifier = match &scroll_behavior {
        Some(behavior) => modifier.then(behavior.search_bar_scroll_behavior(Modifier)),
        None => modifier.then(Modifier),
    }
    .fill_max_width()
    .window_insets_padding(window_insets)
    .semantics(|c| c.set_is_traversal_group(true));

    let input_field: SearchBarSlot = Rc::new(input_field);
    ColorProducerSurface(
        ColorProducer::new(move || app_bar_container_color.get()),
        bar_modifier,
        tonal_elevation = if !is_container_transparent { tonal_elevation } else { Dp(0.0) },
        shadow_elevation = if !is_container_transparent { shadow_elevation } else { Dp(0.0) },
        content = move || {
            let (navigation_icon, actions, input_field, state, shape) =
                (navigation_icon.clone(), actions.clone(), input_field.clone(), state.clone(), shape.clone());
            Row(
                modifier = Modifier.padding_values(content_padding.clone()),
                vertical_alignment = Alignment::CenterVertically,
                content = move || {
                    if let Some(it) = navigation_icon.clone() {
                        Box(
                            modifier = Modifier.padding_sides(start = AppBarWithSearchHorizontalPadding),
                            content = move || {
                                KompozitionLocalProvider(
                                    &[LocalContentColor.provides(colors.app_bar_navigation_icon_color)],
                                    &*it,
                                );
                            },
                        );
                    }
                    let is_visible = !state.expands_to_full_screen()
                        || !state.is_expanded()
                        || state.target_value() == SearchBarValue::Expanded; // prevent flickering
                    let (state, input_field, shape) = (state.clone(), input_field.clone(), shape.clone());
                    Box(
                        modifier = Modifier.weight(1.0).alpha(if is_visible { 1.0 } else { 0.0 }),
                        content = move || {
                            SearchBar(
                                state,
                                move || input_field(),
                                modifier = Modifier
                                    .padding_hv(
                                        horizontal = SearchBarAsTopBarPadding,
                                        vertical = AppBarWithSearchVerticalPadding,
                                    )
                                    .width_in(min = SearchBarMinWidth, max = SearchBarMaxWidth)
                                    .align(Alignment::Center),
                                shape = shape,
                                colors = search_bar_colors,
                                tonal_elevation = if is_container_transparent { tonal_elevation } else { Dp(0.0) },
                                shadow_elevation = if is_container_transparent { shadow_elevation } else { Dp(0.0) },
                            );
                        },
                    );
                    if let Some(it) = actions.clone() {
                        // Wrap the given action icons in a Row.
                        let actions_row = move || {
                            let it = it.clone();
                            Row(
                                horizontal_arrangement = Arrangement::End,
                                vertical_alignment = Alignment::CenterVertically,
                                content = move || it(),
                            );
                        };
                        Box(
                            modifier = Modifier.padding_sides(end = AppBarWithSearchHorizontalPadding),
                            content = move || {
                                KompozitionLocalProvider(
                                    &[LocalContentColor.provides(colors.app_bar_action_icon_color)],
                                    actions_row,
                                );
                            },
                        );
                    }
                },
            );
        },
    );
}

/// The expanded form of a contained search bar of `state` over the whole window (Compose's Expressive
/// `ExpandedFullScreenContainedSearchBar`): while the state is expanded, in a dialog over the window, `input_field`
/// (focused at the first expansion) 8 dp in from the sides on top of `content`, the view fading in over the
/// window (its content fading by the state's content animation); `colors` default to the contained colors of the
/// state ([`SearchBarDefaults`]`.contained_colors(state)`). Back, or a press outside where `properties` say,
/// collapses the state; a back gesture shrinks the view as it goes. `window_insets` pad it and are consumed.
///
/// Kotlin: `ExpandedFullScreenContainedSearchBar(state: SearchBarState, inputField: @Composable () -> Unit, modifier: Modifier = Modifier, collapsedShape: Shape = SearchBarDefaults.inputFieldShape, colors: SearchBarColors = SearchBarDefaults.containedColors(state = state), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, windowInsets: @Composable () -> WindowInsets = { SearchBarDefaults.fullScreenWindowInsets }, properties: DialogProperties = DialogProperties(), content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ExpandedFullScreenContainedSearchBar(
    state: SearchBarState,
    input_field: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SearchBarDefaults::input_field_shape())] collapsed_shape: Arc<dyn Shape>,
    #[default(None)] colors: Option<SearchBarColors>,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(|| SearchBarDefaults::full_screen_window_insets())] window_insets: impl Fn() -> WindowInsets + 'static,
    #[default(DialogProperties::default())] properties: DialogProperties,
    content: impl Fn() + 'static,
) {
    // (Kotlin's default `SearchBarDefaults.containedColors(state = state)` names another parameter: None is it)
    let colors = colors.unwrap_or_else(|| SearchBarDefaults.contained_colors(state.clone()));
    state.set_expands_to_full_screen(true);
    let input_field: SearchBarSlot = Rc::new(input_field);
    let window_insets: Rc<dyn Fn() -> WindowInsets> = Rc::new(window_insets);
    let content: Rc<dyn Fn()> = Rc::new(content);
    let layout_state = state.clone();
    ExpandedFullScreenSearchBarImpl(
        state,
        properties,
        Rc::new(move |focus_requester: FocusRequester, predictive_back_state: PredictiveBackState| {
            let input_field = input_field.clone();
            FullScreenSearchBarLayout(
                layout_state.clone(),
                predictive_back_state,
                Rc::new(move || {
                    let input_field = input_field.clone();
                    Box(
                        modifier = Modifier.focus_requester(&focus_requester),
                        propagate_min_constraints = true,
                        content = move || input_field(),
                    );
                }),
                PaddingValues::hv(FullScreenExpandedHorizontalPadding, Dp(0.0)),
                modifier.clone(),
                collapsed_shape.clone(),
                colors,
                tonal_elevation,
                shadow_elevation,
                window_insets(),
                true,
                content.clone(),
            );
        }),
    );
}

/// The expanded form of a search bar of `state` over the whole window (Compose's `ExpandedFullScreenSearchBar`):
/// while the state is expanded, in a dialog over the window, `input_field` (focused at the first expansion) with
/// a divider in `colors`' divider color and `content` under it, on a surface growing from the collapsed bar's
/// place and size to the window's (its shape from `collapsed_shape` to the full-screen one) as the state expands.
/// Back, or a press outside where `properties` say, collapses the state; a back gesture shrinks the view as it
/// goes. `window_insets` pad it and are consumed.
///
/// Kotlin: `ExpandedFullScreenSearchBar(state: SearchBarState, inputField: @Composable () -> Unit, modifier: Modifier = Modifier, collapsedShape: Shape = SearchBarDefaults.inputFieldShape, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, windowInsets: @Composable () -> WindowInsets = { SearchBarDefaults.fullScreenWindowInsets }, properties: DialogProperties = DialogProperties(), content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ExpandedFullScreenSearchBar(
    state: SearchBarState,
    input_field: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SearchBarDefaults::input_field_shape())] collapsed_shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(|| SearchBarDefaults::full_screen_window_insets())] window_insets: impl Fn() -> WindowInsets + 'static,
    #[default(DialogProperties::default())] properties: DialogProperties,
    content: impl Fn() + 'static,
) {
    state.set_expands_to_full_screen(true);
    let input_field: SearchBarSlot = Rc::new(input_field);
    let window_insets: Rc<dyn Fn() -> WindowInsets> = Rc::new(window_insets);
    let content: Rc<dyn Fn()> = Rc::new(content);
    let layout_state = state.clone();
    ExpandedFullScreenSearchBarImpl(
        state,
        properties,
        Rc::new(move |focus_requester: FocusRequester, predictive_back_state: PredictiveBackState| {
            let input_field = input_field.clone();
            let content = content.clone();
            FullScreenSearchBarLayout(
                layout_state.clone(),
                predictive_back_state,
                Rc::new(move || {
                    let input_field = input_field.clone();
                    Box(
                        modifier = Modifier.focus_requester(&focus_requester),
                        propagate_min_constraints = true,
                        content = move || input_field(),
                    );
                }),
                PaddingValues::all(Dp(0.0)),
                modifier.clone(),
                collapsed_shape.clone(),
                colors,
                tonal_elevation,
                shadow_elevation,
                window_insets(),
                false,
                Rc::new(move || {
                    HorizontalDivider(color = colors.divider_color);
                    content();
                }),
            );
        }),
    );
}

/// What both full-screen forms show while `state` is expanded (Compose's private `ExpandedFullScreenSearchBarImpl`):
/// a [`BasicEdgeToEdgeDialog`] of `properties` collapsing the state when dismissed, `content` given a focus
/// requester (asked for focus at the first expansion) and the dialog's predictive back state; the software
/// keyboard hidden as the state heads for collapsed.
#[kompozable]
fn ExpandedFullScreenSearchBarImpl(
    state: SearchBarState,
    properties: DialogProperties,
    content: Rc<dyn Fn(FocusRequester, PredictiveBackState)>,
) {
    if !state.is_expanded() {
        return;
    }

    let coroutine_scope = remember_coroutine_scope();

    let dismiss_state = state.clone();
    BasicEdgeToEdgeDialog(
        on_dismiss_request = move || launch_animate_to_collapsed(&coroutine_scope, &dismiss_state),
        properties = properties,
        content = move |predictive_back_state: &PredictiveBackState| {
            let focus_requester = remember(FocusRequester::new);

            content(focus_requester.clone(), predictive_back_state.clone());

            // Focus the input field on the first expansion, but no need to re-focus if the focus gets cleared.
            LaunchedEffect((), async move {
                focus_requester.request_focus_with(FocusDirection::Enter);
            });

            // Manually dismiss keyboard when search bar is collapsed. Otherwise, the search bar's window closes and
            // the keyboard disappears suddenly.
            let software_keyboard_controller = LocalSoftwareKeyboardController.current();
            let target_value = state.target_value();
            LaunchedEffect(target_value, async move {
                if target_value == SearchBarValue::Collapsed {
                    software_keyboard_controller.hide();
                }
            });
        },
    );
}

/// The expanded form of a search bar of `state` at its place, its results in a drop-down under it apart from it
/// (Compose's Expressive `ExpandedDockedSearchBarWithGap`): while the state is expanded, in a focusable popup of
/// `properties`, over a scrim of `dropdown_scrim_color` fading in with the state (a press on it collapses the
/// state), `input_field` (focused at the first expansion) in `shape` and `content` in a drop-down of
/// `dropdown_shape` `dropdown_gap_size` under it, sliding in from half its height and fading by the state's
/// content animation. Back collapses the state.
///
/// Kotlin: `ExpandedDockedSearchBarWithGap(state: SearchBarState, inputField: @Composable () -> Unit, modifier: Modifier = Modifier, shape: Shape = SearchBarDefaults.dockedShape, dropdownShape: Shape = SearchBarDefaults.dockedDropdownShape, dropdownGapSize: Dp = SearchBarDefaults.dockedDropdownGapSize, dropdownScrimColor: Color = SearchBarDefaults.dockedDropdownScrimColor, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, properties: PopupProperties = PopupProperties(focusable = true, clippingEnabled = false), content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ExpandedDockedSearchBarWithGap(
    state: SearchBarState,
    input_field: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SearchBarDefaults::docked_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults::docked_dropdown_shape())] dropdown_shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults::DockedDropdownGapSize)] dropdown_gap_size: Dp,
    #[default(SearchBarDefaults::docked_dropdown_scrim_color())] dropdown_scrim_color: Color,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(PopupProperties { focusable: true, clipping_enabled: false, ..PopupProperties::default() })]
    properties: PopupProperties,
    content: impl Fn() + 'static,
) {
    let input_field: SearchBarSlot = Rc::new(input_field);
    let content: Rc<dyn Fn()> = Rc::new(content);
    let layout_state = state.clone();
    ExpandedDockedSearchBarImpl(
        state,
        properties,
        dropdown_scrim_color,
        Rc::new(move |focus_requester: FocusRequester| {
            let input_field = input_field.clone();
            DockedSearchBarLayout(
                layout_state.clone(),
                Rc::new(move || {
                    let input_field = input_field.clone();
                    Box(
                        modifier = Modifier.focus_requester(&focus_requester),
                        propagate_min_constraints = true,
                        content = move || input_field(),
                    );
                }),
                modifier.clone(),
                shape.clone(),
                Some(dropdown_shape.clone()),
                Some(dropdown_gap_size),
                colors,
                tonal_elevation,
                shadow_elevation,
                content.clone(),
            );
        }),
    );
}

/// The expanded form of a search bar of `state` at its place (Compose's `ExpandedDockedSearchBar`): while the
/// state is expanded, in a focusable popup of `properties` at the collapsed bar's place, `input_field` (focused at
/// the first expansion) with a divider in `colors`' divider color and `content` under it, on a [`Surface`] of
/// `shape` as wide as the collapsed bar, growing from its height to two thirds of the window's (at least 240 dp)
/// as the state expands. Back, or a press outside where `properties` say, collapses the state.
///
/// Kotlin: `ExpandedDockedSearchBar(state: SearchBarState, inputField: @Composable () -> Unit, modifier: Modifier = Modifier, shape: Shape = SearchBarDefaults.dockedShape, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, properties: PopupProperties = PopupProperties(focusable = true, clippingEnabled = false), content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ExpandedDockedSearchBar(
    state: SearchBarState,
    input_field: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SearchBarDefaults::docked_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(PopupProperties { focusable: true, clipping_enabled: false, ..PopupProperties::default() })]
    properties: PopupProperties,
    content: impl Fn() + 'static,
) {
    let input_field: SearchBarSlot = Rc::new(input_field);
    let content: Rc<dyn Fn()> = Rc::new(content);
    let layout_state = state.clone();
    ExpandedDockedSearchBarImpl(
        state,
        properties,
        Color::Unspecified,
        Rc::new(move |focus_requester: FocusRequester| {
            let input_field = input_field.clone();
            DockedSearchBarLayout(
                layout_state.clone(),
                Rc::new(move || {
                    let input_field = input_field.clone();
                    Box(
                        modifier = Modifier.focus_requester(&focus_requester),
                        propagate_min_constraints = true,
                        content = move || input_field(),
                    );
                }),
                modifier.clone(),
                shape.clone(),
                None,
                None,
                colors,
                tonal_elevation,
                shadow_elevation,
                content.clone(),
            );
        }),
    );
}

/// What both docked forms show while `state` is expanded (Compose's private `ExpandedDockedSearchBarImpl`): a
/// [`Popup`] of `properties` at the collapsed bar's place (at the window's origin with a scrim: the scrim covers
/// the window, a press on it collapsing the state, the content offset to the bar's place) collapsing the state
/// when dismissed, `content` given a focus requester (asked for focus at the first expansion); the software
/// keyboard hidden as the state heads for collapsed.
#[kompozable]
fn ExpandedDockedSearchBarImpl(
    state: SearchBarState,
    properties: PopupProperties,
    scrim_color: Color,
    content: Rc<dyn Fn(FocusRequester)>,
) {
    if !state.is_expanded() {
        return;
    }

    let has_scrim = scrim_color != Color::Unspecified && scrim_color != Color::Transparent;
    let position_provider: Rc<dyn PopupPositionProvider> =
        Rc::new(DockedPositionProvider { has_scrim, state: state.clone() });
    let scope = remember_coroutine_scope();
    let on_dismiss: Rc<dyn Fn()> = {
        let state = state.clone();
        Rc::new(move || launch_animate_to_collapsed(&scope, &state))
    };

    let dismiss = on_dismiss.clone();
    Popup(
        popup_position_provider = Some(position_provider),
        on_dismiss_request = move || dismiss(),
        properties = properties,
        content = move || {
            let focus_requester = remember(FocusRequester::new);

            if has_scrim {
                let scrim_alpha = scrim_color.alpha() * state.progress();
                let (on_dismiss, offset_state, content, focus_requester) =
                    (on_dismiss.clone(), state.clone(), content.clone(), focus_requester.clone());
                Box(
                    modifier = Modifier
                        .fill_max_size()
                        .background(scrim_color.copy(alpha = scrim_alpha))
                        .clickable(on_click = move || on_dismiss())
                        .offset_px(move |_: &Density| offset_state.collapsed_bounds().top_left()),
                    content = move || content(focus_requester.clone()),
                );
            } else {
                content(focus_requester.clone());
            }

            // Focus the input field on the first expansion, but no need to re-focus if the focus gets cleared.
            LaunchedEffect((), async move {
                focus_requester.request_focus_with(FocusDirection::Enter);
            });

            // Manually dismiss keyboard when search bar is collapsed. Otherwise, the search bar's window closes and
            // the keyboard disappears suddenly.
            let software_keyboard_controller = LocalSoftwareKeyboardController.current();
            let target_value = state.target_value();
            LaunchedEffect(target_value, async move {
                if target_value == SearchBarValue::Collapsed {
                    software_keyboard_controller.hide();
                }
            });
        },
    );
}

/// The docked popup's position (Compose's `object : PopupPositionProvider` of `ExpandedDockedSearchBarImpl`): the
/// window's origin with a scrim, else the collapsed bar's place.
struct DockedPositionProvider {
    has_scrim: bool,
    state: SearchBarState,
}

impl PopupPositionProvider for DockedPositionProvider {
    fn calculate_position(
        &self,
        _anchor_bounds: IntRect,
        _window_size: IntSize,
        _layout_direction: LayoutDirection,
        _popup_content_size: IntSize,
    ) -> IntOffset {
        if self.has_scrim { IntOffset::Zero } else { self.state.collapsed_bounds().top_left() }
    }
}

/// Kotlin's `scope.launch { state.animateToCollapsed() }`.
fn launch_animate_to_collapsed(scope: &CoroutineScope, state: &SearchBarState) {
    let state = state.clone();
    named_args!(scope.launch(async move { state.animate_to_collapsed().await }));
}

// ---------------------------------------------------------------------------------------------------------------
// The bars that expand themselves

/// A search bar that expands itself over the window (Compose's `SearchBar(inputField, expanded, onExpandedChange,
/// …, content)`): `input_field` (typically a [`SearchBarDefaults`]`.InputField…` of the same `expanded` and
/// `on_expanded_change`) on a surface of `shape` (from the input field's shape to the full-screen one as it
/// animates, when it is the default), `colors`' container color, `tonal_elevation` and `shadow_elevation`; while
/// `expanded`, it grows to fill what it is given (the window, as a top bar in a Scaffold), `content` in a Column
/// under the field and a divider in `colors`' divider color, fading in. Back gives `on_expanded_change` false; a
/// back gesture shrinks it as it goes. `window_insets` pad it (animated) and are consumed.
///
/// Kotlin: `SearchBar(inputField: @Composable () -> Unit, expanded: Boolean, onExpandedChange: (Boolean) -> Unit, modifier: Modifier = Modifier, shape: Shape = SearchBarDefaults.inputFieldShape, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, windowInsets: WindowInsets = SearchBarDefaults.windowInsets, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn SearchBarWithContent(
    input_field: impl Fn() + 'static,
    expanded: bool,
    on_expanded_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SearchBarDefaults::input_field_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(SearchBarDefaults::window_insets())] window_insets: WindowInsets,
    content: impl Fn() + 'static,
) {
    let coroutine_scope = remember_coroutine_scope();
    let animation_progress = remember(|| Animatable(if expanded { 1.0f32 } else { 0.0 }));
    let final_back_progress = remember(|| mutable_state_of(f32::NAN));
    let first_back_event = remember(|| mutable_state_of(None::<BackEventCompat>));
    let current_back_event = remember(|| mutable_state_of(None::<BackEventCompat>));

    {
        let (animation_progress, final_back_progress, first_back_event, current_back_event) = (
            animation_progress.clone(),
            final_back_progress.clone(),
            first_back_event.clone(),
            current_back_event.clone(),
        );
        LaunchedEffect(expanded, async move {
            let value = animation_progress.value();
            let animation_in_progress = value > 0.0 && value < 1.0;
            let animation_spec = if animation_in_progress {
                AnimationPredictiveBackExitFloatSpec()
            } else if expanded {
                AnimationEnterFloatSpec()
            } else {
                AnimationExitFloatSpec()
            };
            let target_value = if expanded { 1.0 } else { 0.0 };
            if animation_progress.value() != target_value {
                named_args!(animation_progress.animate_to(target_value, animation_spec = animation_spec)).await;
            }
            if !expanded {
                final_back_progress.set(f32::NAN);
                first_back_event.set(None);
                current_back_event.set(None);
            }
        });
    }

    let mutator_mutex = remember(MutatorMutex::new);
    let on_expanded_change: Rc<dyn Fn(bool)> = Rc::new(on_expanded_change);
    {
        let (animation_progress, final_back_progress, first_back_event, current_back_event) = (
            animation_progress.clone(),
            final_back_progress.clone(),
            first_back_event.clone(),
            current_back_event.clone(),
        );
        PredictiveBackHandler(
            enabled = expanded,
            on_back = move |progress: Flow<BackEventCompat>| {
                let (animation_progress, final_back_progress, first_back_event, current_back_event) = (
                    animation_progress.clone(),
                    final_back_progress.clone(),
                    first_back_event.clone(),
                    current_back_event.clone(),
                );
                let (mutator_mutex, coroutine_scope, on_expanded_change) =
                    (mutator_mutex.clone(), coroutine_scope.clone(), on_expanded_change.clone());
                async move {
                    named_args!(mutator_mutex.mutate(async move {
                        // (Kotlin's `catch (e: CancellationException) { coroutineScope.launch { … } }`: a called-off
                        // gesture's coroutine is dropped here, running nothing more, so what the catch does is launched
                        // in the bar's scope as it goes)
                        let mut restore_if_cancelled = RestoreIfCancelled(Some(RestoreOnCancel {
                            scope: coroutine_scope,
                            animation_progress: animation_progress.clone(),
                            final_back_progress: final_back_progress.clone(),
                            first_back_event: first_back_event.clone(),
                            current_back_event: current_back_event.clone(),
                        }));
                        final_back_progress.set(f32::NAN);
                        let steps = (animation_progress.clone(), first_back_event, current_back_event);
                        progress
                            .collect_async(move |back_event| {
                                let (animation_progress, first_back_event, current_back_event) = steps.clone();
                                async move {
                                    if first_back_event.peek().is_none() {
                                        first_back_event.set(Some(back_event));
                                    }
                                    current_back_event.set(Some(back_event));
                                    let interpolated_progress = PredictiveBack::transform(back_event.progress);
                                    animation_progress.snap_to(1.0 - interpolated_progress).await;
                                }
                            })
                            .await;
                        restore_if_cancelled.0 = None;
                        final_back_progress.set(animation_progress.value());
                        on_expanded_change(false);
                    }))
                    .await;
                }
            },
        );
    }

    SearchBarImpl(
        animation_progress,
        final_back_progress,
        first_back_event,
        current_back_event,
        modifier,
        Rc::new(input_field),
        shape,
        colors,
        tonal_elevation,
        shadow_elevation,
        window_insets,
        Rc::new(content),
    );
}

/// What a cancelled back gesture of [`SearchBarWithContent`] restores (its catch's state).
struct RestoreOnCancel {
    scope: CoroutineScope,
    animation_progress: Animatable<f32, AnimationVector1D>,
    final_back_progress: MutableState<f32>,
    first_back_event: MutableState<Option<BackEventCompat>>,
    current_back_event: MutableState<Option<BackEventCompat>>,
}

/// Launches the bar's animation back to expanded when dropped armed: the gesture called off while it collects.
struct RestoreIfCancelled(Option<RestoreOnCancel>);

impl Drop for RestoreIfCancelled {
    fn drop(&mut self) {
        if let Some(RestoreOnCancel {
            scope,
            animation_progress,
            final_back_progress,
            first_back_event,
            current_back_event,
        }) = self.0.take()
        {
            named_args!(scope.launch(async move {
                named_args!(
                    animation_progress.animate_to(1.0, animation_spec = AnimationPredictiveBackExitFloatSpec())
                )
                .await;
                final_back_progress.set(f32::NAN);
                first_back_event.set(None);
                current_back_event.set(None);
            }));
        }
    }
}

/// A search bar that expands itself in place (Compose's `DockedSearchBar`): `input_field` (typically a
/// [`SearchBarDefaults`]`.InputField…` of the same `expanded` and `on_expanded_change`) on a [`Surface`] of `shape`,
/// 360 dp wide (unless the modifier says otherwise), drawn over what follows it; while `expanded`, a divider in
/// `colors`' divider color and `content` in a Column under the field, between 240 dp and two thirds of the window
/// tall, fading and expanding in. Back gives `on_expanded_change` false.
///
/// Kotlin: `DockedSearchBar(inputField: @Composable () -> Unit, expanded: Boolean, onExpandedChange: (Boolean) -> Unit, modifier: Modifier = Modifier, shape: Shape = SearchBarDefaults.dockedShape, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn DockedSearchBar(
    input_field: impl Fn() + 'static,
    expanded: bool,
    on_expanded_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(SearchBarDefaults::docked_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    content: impl Fn() + 'static,
) {
    let content: Rc<dyn Fn()> = Rc::new(content);
    Surface(
        shape = shape,
        color = colors.container_color,
        content_color = Some(content_color_for(colors.container_color)),
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        modifier = modifier.z_index(1.0).width(SearchBarMinWidth),
        content = move || {
            Column(
                content = move || {
                    input_field();

                    AnimatedVisibility(
                        visible = expanded,
                        enter = DockedEnterTransition(),
                        exit = DockedExitTransition(),
                        content = move |_: &AnimatedVisibilityScope| {
                            let window_container_height = get_window_container_height();
                            let max_height = window_container_height * DockedExpandedTableMaxHeightScreenRatio;
                            let min_height = DockedExpandedTableMinHeight.coerce_at_most(max_height);

                            let content = content.clone();
                            Column(
                                modifier = Modifier.height_in(min = min_height, max = max_height),
                                content = move || {
                                    HorizontalDivider(color = colors.divider_color);
                                    content();
                                },
                            );
                        },
                    );
                },
            );
        },
    );

    BackHandler(enabled = expanded, on_back = move || on_expanded_change(false));
}

/// Compose's deprecated `SearchBar(query: String, …)`: a [`SearchBarWithContent`] of a
/// [`SearchBarDefaults`]`.StringInputField(…)` of these (filling the width, in `colors`' input field colors).
///
/// Kotlin: `SearchBar(query: String, onQueryChange: (String) -> Unit, onSearch: (String) -> Unit, active: Boolean, onActiveChange: (Boolean) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, placeholder: @Composable (() -> Unit)? = null, leadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, shape: Shape = SearchBarDefaults.inputFieldShape, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, windowInsets: WindowInsets = SearchBarDefaults.windowInsets, interactionSource: MutableInteractionSource? = null, content: @Composable ColumnScope.() -> Unit)`
#[deprecated(note = "Use overload which takes inputField as a parameter")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StringSearchBar(
    query: impl Into<String>,
    on_query_change: impl Fn(String) + 'static,
    on_search: impl Fn(String) + 'static,
    active: bool,
    on_active_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] placeholder: Option<Rc<dyn Fn()>>,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(SearchBarDefaults::input_field_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(SearchBarDefaults::window_insets())] window_insets: WindowInsets,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let query: String = query;
    let on_query_change: Rc<dyn Fn(String)> = Rc::new(on_query_change);
    let on_search: Rc<dyn Fn(String)> = Rc::new(on_search);
    let on_active_change: Rc<dyn Fn(bool)> = Rc::new(on_active_change);
    let field_on_active_change = on_active_change.clone();
    SearchBarWithContent(
        move || {
            let (on_query_change, on_search, on_active_change) =
                (on_query_change.clone(), on_search.clone(), field_on_active_change.clone());
            SearchBarDefaults.StringInputField(
                query.clone(),
                move |it: String| on_query_change(it),
                move |it: String| on_search(it),
                active,
                move |it: bool| on_active_change(it),
                modifier = Modifier.fill_max_width(),
                enabled = enabled,
                placeholder = placeholder.clone(),
                leading_icon = leading_icon.clone(),
                trailing_icon = trailing_icon.clone(),
                colors = colors.input_field_colors,
                interaction_source = interaction_source.clone(),
            );
        },
        active,
        move |it: bool| on_active_change(it),
        modifier = modifier,
        shape = shape,
        colors = colors,
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        window_insets = window_insets,
        content = content,
    );
}

/// Compose's deprecated `DockedSearchBar(query: String, …)`: a [`DockedSearchBar`] of a
/// [`SearchBarDefaults`]`.StringInputField(…)` of these (filling the width, in `colors`' input field colors).
///
/// Kotlin: `DockedSearchBar(query: String, onQueryChange: (String) -> Unit, onSearch: (String) -> Unit, active: Boolean, onActiveChange: (Boolean) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, placeholder: @Composable (() -> Unit)? = null, leadingIcon: @Composable (() -> Unit)? = null, trailingIcon: @Composable (() -> Unit)? = null, shape: Shape = SearchBarDefaults.dockedShape, colors: SearchBarColors = SearchBarDefaults.colors(), tonalElevation: Dp = SearchBarDefaults.TonalElevation, shadowElevation: Dp = SearchBarDefaults.ShadowElevation, interactionSource: MutableInteractionSource? = null, content: @Composable ColumnScope.() -> Unit)`
#[deprecated(note = "Use overload which takes inputField as a parameter")]
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn StringDockedSearchBar(
    query: impl Into<String>,
    on_query_change: impl Fn(String) + 'static,
    on_search: impl Fn(String) + 'static,
    active: bool,
    on_active_change: impl Fn(bool) + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] placeholder: Option<Rc<dyn Fn()>>,
    #[default(None)] leading_icon: Option<Rc<dyn Fn()>>,
    #[default(None)] trailing_icon: Option<Rc<dyn Fn()>>,
    #[default(SearchBarDefaults::docked_shape())] shape: Arc<dyn Shape>,
    #[default(SearchBarDefaults.colors())] colors: SearchBarColors,
    #[default(SearchBarDefaults::TonalElevation)] tonal_elevation: Dp,
    #[default(SearchBarDefaults::ShadowElevation)] shadow_elevation: Dp,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    content: impl Fn() + 'static,
) {
    let query: String = query;
    let on_query_change: Rc<dyn Fn(String)> = Rc::new(on_query_change);
    let on_search: Rc<dyn Fn(String)> = Rc::new(on_search);
    let on_active_change: Rc<dyn Fn(bool)> = Rc::new(on_active_change);
    let field_on_active_change = on_active_change.clone();
    DockedSearchBar(
        move || {
            let (on_query_change, on_search, on_active_change) =
                (on_query_change.clone(), on_search.clone(), field_on_active_change.clone());
            SearchBarDefaults.StringInputField(
                query.clone(),
                move |it: String| on_query_change(it),
                move |it: String| on_search(it),
                active,
                move |it: bool| on_active_change(it),
                modifier = Modifier.fill_max_width(),
                enabled = enabled,
                placeholder = placeholder.clone(),
                leading_icon = leading_icon.clone(),
                trailing_icon = trailing_icon.clone(),
                colors = colors.input_field_colors,
                interaction_source = interaction_source.clone(),
            );
        },
        active,
        move |it: bool| on_active_change(it),
        modifier = modifier,
        shape = shape,
        colors = colors,
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        content = content,
    );
}

// ---------------------------------------------------------------------------------------------------------------
// SearchBarValue, SearchBarState

/// Possible values of [`SearchBarState`] (Compose's enum class `SearchBarValue`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SearchBarValue {
    /// The state of the search bar when it is collapsed.
    Collapsed,
    /// The state of the search bar when it is expanded.
    Expanded,
}

impl fmt::Display for SearchBarValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// The state of a search bar (Compose's `SearchBarState`): how far it is expanded ([`progress`]
/// (SearchBarState::progress), 0 collapsed, 1 expanded), the value it is heading for and the one it is at, the
/// animations between them, where its collapsed bar is ([`collapsed_coords`](SearchBarState::collapsed_coords)).
/// A cheap handle: clones are the same state (equal when they are).
#[derive(Clone)]
pub struct SearchBarState {
    inner: Rc<SearchBarStateInner>,
}

struct SearchBarStateInner {
    animatable: Animatable<f32, AnimationVector1D>,
    content_animatable: Animatable<f32, AnimationVector1D>,
    animation_spec_for_expand: Arc<dyn AnimationSpec<f32>>,
    animation_spec_for_collapse: Arc<dyn AnimationSpec<f32>>,
    animation_spec_for_content_fade_in: Arc<dyn AnimationSpec<f32>>,
    animation_spec_for_content_fade_out: Arc<dyn AnimationSpec<f32>>,
    expands_to_full_screen: MutableState<bool>,
    // (Compose's `collapsedCoords by mutableStateOf(null)`: layout coordinates aren't `Send`, so they are kept
    // beside a snapshot state counted up when they change, read with them — observed as Compose's state is; set
    // equal, nothing changes, as its structural policy has it)
    collapsed_coords: RefCell<Option<LayoutCoordinates>>,
    collapsed_coords_version: MutableState<u64>,
    current_value: DerivedState<SearchBarValue>,
}

impl PartialEq for SearchBarState {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for SearchBarState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SearchBarState(currentValue={:?}, progress={})", self.current_value(), self.progress())
    }
}

/// A search bar's state (Compose's `SearchBarState(initialValue, animationSpecForExpand, animationSpecForCollapse)`
/// and `SearchBarState(initialValue, animationSpecForExpand, animationSpecForCollapse, animationSpecForContentFadeIn,
/// animationSpecForContentFadeOut)`: one function, Kotlin's three-parameter constructor the five-parameter one of
/// snapping fades): at `initial_value`, expanding along `animation_spec_for_expand` and collapsing along
/// `animation_spec_for_collapse`, its content (a contained or docked view's) fading in and out along the last two.
///
/// Kotlin: `SearchBarState(initialValue: SearchBarValue, animationSpecForExpand: AnimationSpec<Float>, animationSpecForCollapse: AnimationSpec<Float>, animationSpecForContentFadeIn: AnimationSpec<Float> = snap(), animationSpecForContentFadeOut: AnimationSpec<Float> = snap())`
#[named(type)]
pub fn SearchBarState(
    initial_value: SearchBarValue,
    animation_spec_for_expand: Arc<dyn AnimationSpec<f32>>,
    animation_spec_for_collapse: Arc<dyn AnimationSpec<f32>>,
    #[default(Arc::new(snap::<f32>()))] animation_spec_for_content_fade_in: Arc<dyn AnimationSpec<f32>>,
    #[default(Arc::new(snap::<f32>()))] animation_spec_for_content_fade_out: Arc<dyn AnimationSpec<f32>>,
) -> SearchBarState {
    SearchBarState::new(
        initial_value,
        animation_spec_for_expand,
        animation_spec_for_collapse,
        animation_spec_for_content_fade_in,
        animation_spec_for_content_fade_out,
    )
}

/// The value of a collapsed and an expanded state's animation (Compose's private companion `Collapsed`,
/// `Expanded`).
const Collapsed: f32 = 0.0;
const Expanded: f32 = 1.0;

impl SearchBarState {
    /// [`SearchBarState()`](fn@SearchBarState) of all its parameters.
    pub(crate) fn new(
        initial_value: SearchBarValue,
        animation_spec_for_expand: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_collapse: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_content_fade_in: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_content_fade_out: Arc<dyn AnimationSpec<f32>>,
    ) -> SearchBarState {
        let initial = if initial_value == SearchBarValue::Expanded { Expanded } else { Collapsed };
        SearchBarState::of_animatables(
            Animatable(initial),
            Animatable(initial),
            animation_spec_for_expand,
            animation_spec_for_collapse,
            animation_spec_for_content_fade_in,
            animation_spec_for_content_fade_out,
        )
    }

    /// Compose's private constructor of the animatables.
    fn of_animatables(
        animatable: Animatable<f32, AnimationVector1D>,
        content_animatable: Animatable<f32, AnimationVector1D>,
        animation_spec_for_expand: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_collapse: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_content_fade_in: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_content_fade_out: Arc<dyn AnimationSpec<f32>>,
    ) -> SearchBarState {
        let current_value = {
            let animatable = animatable.clone();
            derived_state_of(move || {
                let threshold = 0.02; // tolerance for spring anim
                if animatable.value() <= Collapsed + threshold {
                    SearchBarValue::Collapsed
                } else {
                    SearchBarValue::Expanded
                }
            })
        };
        SearchBarState {
            inner: Rc::new(SearchBarStateInner {
                animatable,
                content_animatable,
                animation_spec_for_expand,
                animation_spec_for_collapse,
                animation_spec_for_content_fade_in,
                animation_spec_for_content_fade_out,
                expands_to_full_screen: mutable_state_of(false),
                collapsed_coords: RefCell::new(None),
                collapsed_coords_version: mutable_state_of(0),
                current_value,
            }),
        }
    }

    /// Whether it expands to the whole window (Compose's internal `expandsToFullScreen`, a state): what its
    /// full-screen forms set.
    pub(crate) fn expands_to_full_screen(&self) -> bool {
        self.inner.expands_to_full_screen.get()
    }

    pub(crate) fn set_expands_to_full_screen(&self, value: bool) {
        self.inner.expands_to_full_screen.set(value);
    }

    /// Where the collapsed bar is (Compose's `collapsedCoords`, a state): what the bar of this state sets as it is
    /// placed, what its expanded forms grow from; none until then.
    pub fn collapsed_coords(&self) -> Option<LayoutCoordinates> {
        self.inner.collapsed_coords_version.get();
        self.inner.collapsed_coords.borrow().clone()
    }

    /// Sets [`collapsed_coords`](SearchBarState::collapsed_coords): its readers are told (if they changed).
    pub fn set_collapsed_coords(&self, coords: Option<LayoutCoordinates>) {
        if *self.inner.collapsed_coords.borrow() == coords {
            return;
        }
        *self.inner.collapsed_coords.borrow_mut() = coords;
        self.inner.collapsed_coords_version.set(self.inner.collapsed_coords_version.peek() + 1);
    }

    /// How far it is expanded, 0 collapsed to 1 expanded (Compose's `progress`, a state read).
    pub fn progress(&self) -> f32 {
        self.inner.animatable.value().clamp(0.0, 1.0)
    }

    /// How far its content has faded in (Compose's internal `contentProgress`).
    pub(crate) fn content_progress(&self) -> f32 {
        self.inner.content_animatable.value().clamp(0.0, 1.0)
    }

    /// Whether it is animating (Compose's `isAnimating`).
    pub fn is_animating(&self) -> bool {
        self.inner.animatable.is_running()
    }

    /// Whether it is going to be expanded or collapsed (Compose's `targetValue`).
    pub fn target_value(&self) -> SearchBarValue {
        if self.inner.animatable.target_value() == Expanded {
            SearchBarValue::Expanded
        } else {
            SearchBarValue::Collapsed
        }
    }

    /// The value it is at (Compose's `currentValue`): expanded once its animation is past a little of the way.
    pub fn current_value(&self) -> SearchBarValue {
        self.inner.current_value.get()
    }

    /// Expanded (Compose's private `isExpanded`).
    pub(crate) fn is_expanded(&self) -> bool {
        self.current_value() == SearchBarValue::Expanded
    }

    /// Its animation's value, not limited to 0..1 (Compose's internal `animatable.value`).
    pub(crate) fn animatable_value(&self) -> f32 {
        self.inner.animatable.value()
    }

    /// Its animation's value as a state shareable with a shape.
    fn shared_animatable_value(&self) -> SharedState<f32> {
        self.inner.animatable.as_shared_state()
    }

    /// Its expanding spec (Compose's internal `animationSpecForExpand`).
    pub(crate) fn animation_spec_for_expand(&self) -> Arc<dyn AnimationSpec<f32>> {
        self.inner.animation_spec_for_expand.clone()
    }

    /// Its collapsing spec (Compose's internal `animationSpecForCollapse`).
    pub(crate) fn animation_spec_for_collapse(&self) -> Arc<dyn AnimationSpec<f32>> {
        self.inner.animation_spec_for_collapse.clone()
    }

    /// Where the collapsed bar is in the window (Compose's private `collapsedBounds`): zero unless its coordinates
    /// are set and attached.
    pub(crate) fn collapsed_bounds(&self) -> IntRect {
        self.collapsed_coords()
            .filter(|it| it.is_attached())
            .map(|it| IntRect::from_offset_size(it.position_in_window().round(), it.size()))
            .unwrap_or(IntRect::Zero)
    }

    /// Animates it to expanded (Compose's `animateToExpanded`): its value along the expanding spec, its content's
    /// along the fade-in one, together.
    pub fn animate_to_expanded(&self) -> impl Future<Output = ()> + 'static {
        let inner = self.inner.clone();
        coroutine_scope(move |scope| async move {
            let (animatable, spec) = (inner.animatable.clone(), inner.animation_spec_for_expand.clone());
            named_args!(scope.launch(async move {
                named_args!(animatable.animate_to(Expanded, animation_spec = spec)).await;
            }));
            let (content, spec) = (inner.content_animatable.clone(), inner.animation_spec_for_content_fade_in.clone());
            named_args!(scope.launch(async move {
                named_args!(content.animate_to(Expanded, animation_spec = spec)).await;
            }));
        })
    }

    /// Animates it to collapsed (Compose's `animateToCollapsed`): its content's value along the fade-out spec, its
    /// value along the collapsing one, together.
    pub fn animate_to_collapsed(&self) -> impl Future<Output = ()> + 'static {
        let inner = self.inner.clone();
        coroutine_scope(move |scope| async move {
            let (content, spec) = (inner.content_animatable.clone(), inner.animation_spec_for_content_fade_out.clone());
            named_args!(scope.launch(async move {
                named_args!(content.animate_to(Collapsed, animation_spec = spec)).await;
            }));
            let (animatable, spec) = (inner.animatable.clone(), inner.animation_spec_for_collapse.clone());
            named_args!(scope.launch(async move {
                named_args!(animatable.animate_to(Collapsed, animation_spec = spec)).await;
            }));
        })
    }

    /// Snaps it to `fraction` of the way expanded, without animation (Compose's `snapTo`): 0 collapsed, 1 expanded.
    pub fn snap_to(&self, fraction: f32) -> impl Future<Output = ()> + 'static {
        self.inner.animatable.snap_to(fraction)
    }

    /// The default saver of a search bar's state (Compose's `SearchBarState.Saver(animationSpecForExpand,
    /// animationSpecForCollapse)`): its progress and content progress saved, a state of them restored, of these
    /// specs and snapping fades.
    pub fn Saver(
        animation_spec_for_expand: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_collapse: Arc<dyn AnimationSpec<f32>>,
    ) -> Saver<SearchBarState> {
        SearchBarState::SaverWithContentFade(
            animation_spec_for_expand,
            animation_spec_for_collapse,
            Arc::new(snap::<f32>()),
            Arc::new(snap::<f32>()),
        )
    }

    /// The default saver of a search bar's state with fading content (Compose's `SearchBarState.Saver(
    /// animationSpecForExpand, animationSpecForCollapse, animationSpecForContentFadeIn,
    /// animationSpecForContentFadeOut)`).
    ///
    /// Kotlin: `Saver(animationSpecForExpand: AnimationSpec<Float>, animationSpecForCollapse: AnimationSpec<Float>, animationSpecForContentFadeIn: AnimationSpec<Float>, animationSpecForContentFadeOut: AnimationSpec<Float>): Saver<SearchBarState, *>`
    pub fn SaverWithContentFade(
        animation_spec_for_expand: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_collapse: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_content_fade_in: Arc<dyn AnimationSpec<f32>>,
        animation_spec_for_content_fade_out: Arc<dyn AnimationSpec<f32>>,
    ) -> Saver<SearchBarState> {
        Saver::new(
            |it: &SearchBarState| {
                SavedValue::list([
                    SavedValue::Double(it.progress() as f64),
                    SavedValue::Double(it.content_progress() as f64),
                ])
            },
            move |saved| {
                let list = saved.as_list()?;
                let progress = list.first()?.as_f64()? as f32;
                let content_progress = list.get(1)?.as_f64()? as f32;
                Some(SearchBarState::of_animatables(
                    Animatable(progress),
                    Animatable(content_progress),
                    animation_spec_for_expand.clone(),
                    animation_spec_for_collapse.clone(),
                    animation_spec_for_content_fade_in.clone(),
                    animation_spec_for_content_fade_out.clone(),
                ))
            },
        )
    }
}

/// A search bar's state, saved and restored by [`SearchBarState::Saver`], made again when `initial_value` or the
/// specs change (Compose's `rememberSearchBarState`): collapsed unless `initial_value` says; expanding along the
/// theme's slow spatial motion and collapsing along its default spatial one by default.
#[kompozable]
pub fn remember_search_bar_state(
    #[default(SearchBarValue::Collapsed)] initial_value: SearchBarValue,
    #[default(MotionSchemeKeyTokens::SlowSpatial.value::<f32>())] animation_spec_for_expand: Arc<
        dyn AnimationSpec<f32>,
    >,
    #[default(MotionSchemeKeyTokens::DefaultSpatial.value::<f32>())] animation_spec_for_collapse: Arc<
        dyn AnimationSpec<f32>,
    >,
) -> SearchBarState {
    let saver = SearchBarState::Saver(animation_spec_for_expand.clone(), animation_spec_for_collapse.clone());
    remember_saveable_keyed_with(
        (initial_value, animation_spec_for_expand.clone(), animation_spec_for_collapse.clone()),
        &saver,
        move || {
            SearchBarState::new(
                initial_value,
                animation_spec_for_expand,
                animation_spec_for_collapse,
                Arc::new(snap::<f32>()),
                Arc::new(snap::<f32>()),
            )
        },
    )
}

/// A contained search bar's state (Compose's Expressive `rememberContainedSearchBarState`): as
/// [`remember_search_bar_state`]'s, expanding and collapsing along the theme's fast spatial motion, its content
/// fading in and out by a short tween by default.
#[kompozable]
pub fn remember_contained_search_bar_state(
    #[default(SearchBarValue::Collapsed)] initial_value: SearchBarValue,
    #[default(MotionSchemeKeyTokens::FastSpatial.value::<f32>())] animation_spec_for_expand: Arc<
        dyn AnimationSpec<f32>,
    >,
    #[default(MotionSchemeKeyTokens::FastSpatial.value::<f32>())] animation_spec_for_collapse: Arc<
        dyn AnimationSpec<f32>,
    >,
    #[default(AnimationForContentFadeInSpec())] animation_spec_for_content_fade_in: Arc<dyn AnimationSpec<f32>>,
    #[default(AnimationForContentFadeOutSpec())] animation_spec_for_content_fade_out: Arc<dyn AnimationSpec<f32>>,
) -> SearchBarState {
    remember_fading_search_bar_state(
        initial_value,
        animation_spec_for_expand,
        animation_spec_for_collapse,
        animation_spec_for_content_fade_in,
        animation_spec_for_content_fade_out,
    )
}

/// A docked search bar's state whose results drop down apart from it (Compose's Expressive
/// `rememberWithGapSearchBarState`): as [`remember_search_bar_state`]'s, expanding along the theme's default
/// spatial motion and collapsing along its fast spatial one, its content fading in and out by a short tween by
/// default.
#[kompozable]
pub fn remember_with_gap_search_bar_state(
    #[default(SearchBarValue::Collapsed)] initial_value: SearchBarValue,
    #[default(MotionSchemeKeyTokens::DefaultSpatial.value::<f32>())] animation_spec_for_expand: Arc<
        dyn AnimationSpec<f32>,
    >,
    #[default(MotionSchemeKeyTokens::FastSpatial.value::<f32>())] animation_spec_for_collapse: Arc<
        dyn AnimationSpec<f32>,
    >,
    #[default(AnimationForContentFadeInSpec())] animation_spec_for_content_fade_in: Arc<dyn AnimationSpec<f32>>,
    #[default(AnimationForContentFadeOutSpec())] animation_spec_for_content_fade_out: Arc<dyn AnimationSpec<f32>>,
) -> SearchBarState {
    remember_fading_search_bar_state(
        initial_value,
        animation_spec_for_expand,
        animation_spec_for_collapse,
        animation_spec_for_content_fade_in,
        animation_spec_for_content_fade_out,
    )
}

/// The `rememberSaveable` both fading states' remember functions are made of.
#[kompozable]
fn remember_fading_search_bar_state(
    initial_value: SearchBarValue,
    animation_spec_for_expand: Arc<dyn AnimationSpec<f32>>,
    animation_spec_for_collapse: Arc<dyn AnimationSpec<f32>>,
    animation_spec_for_content_fade_in: Arc<dyn AnimationSpec<f32>>,
    animation_spec_for_content_fade_out: Arc<dyn AnimationSpec<f32>>,
) -> SearchBarState {
    let saver = SearchBarState::SaverWithContentFade(
        animation_spec_for_expand.clone(),
        animation_spec_for_collapse.clone(),
        animation_spec_for_content_fade_in.clone(),
        animation_spec_for_content_fade_out.clone(),
    );
    remember_saveable_keyed_with(
        (
            initial_value,
            animation_spec_for_expand.clone(),
            animation_spec_for_collapse.clone(),
            animation_spec_for_content_fade_in.clone(),
            animation_spec_for_content_fade_out.clone(),
        ),
        &saver,
        move || {
            SearchBarState::new(
                initial_value,
                animation_spec_for_expand,
                animation_spec_for_collapse,
                animation_spec_for_content_fade_in,
                animation_spec_for_content_fade_out,
            )
        },
    )
}

// ---------------------------------------------------------------------------------------------------------------
// SearchBarScrollBehavior

/// How an [`AppBarWithSearch`] moves as content scrolls under it (Compose's interface `SearchBarScrollBehavior`):
/// its offset (0 down to its limit, minus its height: gone), how far the content has scrolled, the nested scroll
/// connection to give the scrolling content's parent (`Modifier.nested_scroll(behavior.nested_scroll_connection())`)
/// and the modifier that moves the bar by it. [`SearchBarDefaults`]`.enter_always_search_bar_scroll_behavior()`
/// makes one.
pub trait SearchBarScrollBehavior {
    /// How far up the bar has slid, px, between [`scroll_offset_limit`](Self::scroll_offset_limit) and 0 (Compose's
    /// `scrollOffset`).
    fn scroll_offset(&self) -> f32;
    /// Sets it, kept between the limit and 0.
    fn set_scroll_offset(&self, offset: f32);
    /// The furthest the bar slides up, px (Compose's `scrollOffsetLimit`): minus its height.
    fn scroll_offset_limit(&self) -> f32;
    fn set_scroll_offset_limit(&self, limit: f32);
    /// How far the content under the bar has scrolled, px (Compose's `contentOffset`).
    fn content_offset(&self) -> f32;
    fn set_content_offset(&self, offset: f32);
    /// What the scrolling content's parent takes (Compose's `nestedScrollConnection`).
    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection>;
    /// `modifier` moving the bar it modifies by this behavior (Compose's member extension
    /// `Modifier.searchBarScrollBehavior()`).
    fn search_bar_scroll_behavior(&self, modifier: Modifier) -> Modifier;
}

/// How much of the bar content overlaps (Compose's internal `SearchBarScrollBehavior.overlappedFraction`).
pub(crate) fn overlapped_fraction(behavior: &dyn SearchBarScrollBehavior) -> f32 {
    let limit = behavior.scroll_offset_limit();
    if limit != 0.0 { 1.0 - ((limit - behavior.content_offset()).clamp(limit, 0.0) / limit) } else { 0.0 }
}

/// The scroll behavior that slides the bar away as content scrolls up and back as soon as it scrolls down
/// (Compose's private `EnterAlwaysSearchBarScrollBehavior`).
#[derive(Clone)]
pub(crate) struct EnterAlwaysSearchBarScrollBehavior(Rc<EnterAlwaysInner>);

struct EnterAlwaysInner {
    scroll_offset: MutableState<f32>,
    scroll_offset_limit: MutableState<f32>,
    content_offset: MutableState<f32>,
    can_scroll: Rc<dyn Fn() -> bool>,
    reverse_layout: bool,
    snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
    fling_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
}

impl EnterAlwaysSearchBarScrollBehavior {
    pub(crate) fn new(
        initial_offset: f32,
        initial_offset_limit: f32,
        initial_content_offset: f32,
        can_scroll: Rc<dyn Fn() -> bool>,
        reverse_layout: bool,
        snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
        fling_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    ) -> EnterAlwaysSearchBarScrollBehavior {
        EnterAlwaysSearchBarScrollBehavior(Rc::new(EnterAlwaysInner {
            scroll_offset: mutable_state_of(initial_offset),
            scroll_offset_limit: mutable_state_of(initial_offset_limit),
            content_offset: mutable_state_of(initial_content_offset),
            can_scroll,
            reverse_layout,
            snap_animation_spec,
            fling_animation_spec,
        }))
    }

    fn can_scroll(&self) -> bool {
        (self.0.can_scroll)()
    }

    /// Compose's `settleSearchBar`: past a sliver and not gone, a fling's velocity carries the bar on (decaying)
    /// and the bar snaps to shown or gone, whichever is nearer; what velocity is left.
    fn settle_search_bar(&self, velocity: f32) -> impl Future<Output = Velocity> + 'static {
        let this = self.clone();
        async move {
            // Check if the search bar scroll is at a scroll limit. If so, no need to settle the search bar, and just
            // return Velocity.Zero. Note that we don't check for 0f due to float precision with the scrollFraction
            // calculation.
            let limit = this.scroll_offset_limit();
            let scroll_fraction = if limit != 0.0 { this.scroll_offset() / limit } else { 0.0 };
            if scroll_fraction < 0.01 || scroll_fraction == 1.0 {
                return Velocity::Zero;
            }
            let remaining_velocity = Rc::new(Cell::new(velocity));
            // In case there is an initial velocity that was left after a previous user fling, animate to continue the
            // motion to scroll the search bar.
            if velocity.abs() > 1.0 {
                let last_value = Rc::new(Cell::new(0.0f32));
                let (behavior, remaining) = (this.clone(), remaining_velocity.clone());
                let block: Box<dyn for<'a> FnMut(&'a kompoz_animation::AnimationScope<f32, AnimationVector1D>)> =
                    Box::new(move |s| {
                        let delta = s.value() - last_value.get();
                        let initial_scroll_offset = behavior.scroll_offset();
                        behavior.set_scroll_offset(initial_scroll_offset + delta);
                        let consumed = (initial_scroll_offset - behavior.scroll_offset()).abs();
                        last_value.set(s.value());
                        remaining.set(s.velocity());
                        // avoid rounding errors and stop if anything is unconsumed
                        if (delta - consumed).abs() > 0.5 {
                            s.cancel_animation();
                        }
                    });
                let state: AnimationState<f32, AnimationVector1D> =
                    named_args!(float_animation_state(0.0, initial_velocity = velocity));
                named_args!(state.animate_decay(this.0.fling_animation_spec.clone(), block = block)).await;
            }
            let (offset, limit) = (this.scroll_offset(), this.scroll_offset_limit());
            if limit < offset && offset < 0.0 {
                let behavior = this.clone();
                let block: Box<dyn for<'a> FnMut(&'a kompoz_animation::AnimationScope<f32, AnimationVector1D>)> =
                    Box::new(move |s| behavior.set_scroll_offset(s.value()));
                let state: AnimationState<f32, AnimationVector1D> = float_animation_state(offset);
                named_args!(state.animate_to(
                    if scroll_fraction < 0.5 { 0.0 } else { limit },
                    animation_spec = this.0.snap_animation_spec.clone(),
                    block = block,
                ))
                .await;
            }

            Velocity::new(0.0, remaining_velocity.get())
        }
    }

    /// Compose's companion `Saver(canScroll, snapAnimationSpec, flingAnimationSpec)`: the offsets and whether the
    /// layout is reversed saved, a behavior of them restored.
    pub(crate) fn saver(
        can_scroll: Rc<dyn Fn() -> bool>,
        snap_animation_spec: Arc<dyn AnimationSpec<f32>>,
        fling_animation_spec: Arc<dyn DecayAnimationSpec<f32>>,
    ) -> Saver<EnterAlwaysSearchBarScrollBehavior> {
        Saver::new(
            |it: &EnterAlwaysSearchBarScrollBehavior| {
                SavedValue::list([
                    SavedValue::Double(it.scroll_offset() as f64),
                    SavedValue::Double(it.scroll_offset_limit() as f64),
                    SavedValue::Double(it.content_offset() as f64),
                    SavedValue::Bool(it.0.reverse_layout),
                ])
            },
            move |saved| {
                let list = saved.as_list()?;
                Some(EnterAlwaysSearchBarScrollBehavior::new(
                    list.first()?.as_f64()? as f32,
                    list.get(1)?.as_f64()? as f32,
                    list.get(2)?.as_f64()? as f32,
                    can_scroll.clone(),
                    list.get(3)?.as_bool()?,
                    snap_animation_spec.clone(),
                    fling_animation_spec.clone(),
                ))
            },
        )
    }
}

impl SearchBarScrollBehavior for EnterAlwaysSearchBarScrollBehavior {
    fn scroll_offset(&self) -> f32 {
        self.0.scroll_offset.get()
    }

    fn set_scroll_offset(&self, offset: f32) {
        self.0.scroll_offset.set(offset.clamp_kt(self.scroll_offset_limit(), 0.0));
    }

    fn scroll_offset_limit(&self) -> f32 {
        self.0.scroll_offset_limit.get()
    }

    fn set_scroll_offset_limit(&self, limit: f32) {
        self.0.scroll_offset_limit.set(limit);
    }

    fn content_offset(&self) -> f32 {
        self.0.content_offset.get()
    }

    fn set_content_offset(&self, offset: f32) {
        self.0.content_offset.set(offset);
    }

    fn search_bar_scroll_behavior(&self, modifier: Modifier) -> Modifier {
        let (on_delta, on_stopped, measured, sized) = (self.clone(), self.clone(), self.clone(), self.clone());
        let on_drag_stopped: OnDragStopped = Rc::new(move |_: CoroutineScope, velocity: f32| {
            let settle = on_stopped.settle_search_bar(velocity);
            Box::pin(async move {
                settle.await;
            }) as LocalFuture<'static, ()>
        });
        named_args!(modifier.draggable(
            state = DraggableState(move |delta: f32| on_delta.set_scroll_offset(on_delta.scroll_offset() + delta)),
            orientation = Orientation::Vertical,
            on_drag_stopped = Some(on_drag_stopped),
            enabled = self.can_scroll(),
        ))
        .clip_to_bounds()
        .layout(move |scope: &MeasureScope, measurable: &dyn Measurable, constraints: Constraints| {
            let placeable = measurable.measure(constraints);
            let scroll_offset = measured.scroll_offset().round() as i32;
            let scrolled_height = (placeable.height() + scroll_offset).max(0);
            scope.layout(placeable.width(), scrolled_height, move |p| {
                p.place_with_layer(&placeable, 0, scroll_offset, |_: &mut GraphicsLayerScope| {});
            })
        })
        .on_size_changed(move |size: IntSize| sized.set_scroll_offset_limit(-(size.height as f32)))
    }

    fn nested_scroll_connection(&self) -> Rc<dyn NestedScrollConnection> {
        Rc::new(EnterAlwaysConnection(self.clone()))
    }
}

/// The behavior's nested scroll connection (Compose's `object : NestedScrollConnection`).
struct EnterAlwaysConnection(EnterAlwaysSearchBarScrollBehavior);

impl NestedScrollConnection for EnterAlwaysConnection {
    fn on_pre_scroll(&self, available: Offset, _source: NestedScrollSource) -> Offset {
        let b = &self.0;
        if !b.can_scroll() {
            return Offset::Zero;
        }
        let prev_scroll_offset = b.scroll_offset();
        b.set_scroll_offset(b.scroll_offset() + available.y);
        // The scrollOffset is coerced between scrollOffsetLimit and 0, so we check if its value was actually
        // changed after available.y is added. If so, the search bar is currently  in between scroll states.
        // Note: when the content is set with reversed layout, we always return Offset.Zero.
        if !b.0.reverse_layout && prev_scroll_offset != b.scroll_offset() {
            Offset::new(0.0, available.y)
        } else {
            Offset::Zero
        }
    }

    fn on_post_scroll(&self, consumed: Offset, available: Offset, _source: NestedScrollSource) -> Offset {
        let b = &self.0;
        if !b.can_scroll() {
            return Offset::Zero;
        }
        if b.0.reverse_layout && available.y > 0.0 {
            // In a reversed layout, consume scroll if it's a pull down to reveal the search bar but not if it's a
            // pull up to hide.
            b.set_scroll_offset(b.scroll_offset() + available.y);
            b.set_content_offset(b.content_offset() + available.y);
            return Offset::new(0.0, available.y);
        }
        if !b.0.reverse_layout {
            b.set_scroll_offset(b.scroll_offset() + consumed.y);
            b.set_content_offset(b.content_offset() + consumed.y);
        }
        Offset::Zero
    }

    fn on_post_fling(&self, _consumed: Velocity, available: Velocity) -> FlingFuture<'_> {
        Box::pin(async move {
            if !self.0.can_scroll() {
                return Velocity::Zero;
            }
            self.0.settle_search_bar(available.y).await
        })
    }
}

/// Kotlin's `coerceIn(min, max)`: `min` if under it, `max` if over it (no panic for `min > max`, Kotlin throws
/// only where `min > max`, which a bar's limit under 0 never is).
trait ClampKt {
    fn clamp_kt(self, min: f32, max: f32) -> f32;
}

impl ClampKt for f32 {
    fn clamp_kt(self, min: f32, max: f32) -> f32 {
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The implementation

/// The bar of [`SearchBarWithContent`] (Compose's internal `SearchBarImpl`): the surface (its shape animated from
/// the input field's to the full screen one while it is the default) behind the input field, the content (with a
/// divider) fading in under it while expanded, laid out by [`SearchBarLayout`].
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn SearchBarImpl(
    animation_progress: Animatable<f32, AnimationVector1D>,
    final_back_progress: MutableState<f32>,
    first_back_event: MutableState<Option<BackEventCompat>>,
    current_back_event: MutableState<Option<BackEventCompat>>,
    modifier: Modifier,
    input_field: SearchBarSlot,
    shape: Arc<dyn Shape>,
    colors: SearchBarColors,
    tonal_elevation: Dp,
    shadow_elevation: Dp,
    window_insets: WindowInsets,
    content: Rc<dyn Fn()>,
) {
    let density = LocalDensity.current();

    let default_input_field_shape = SearchBarDefaults::input_field_shape();
    let default_full_screen_shape = SearchBarDefaults::full_screen_shape();
    let use_full_screen_shape = {
        let progress = animation_progress.clone();
        remember(move || derived_state_of(move || progress.value() == 1.0))
    }
    .get();
    let animated_shape: Arc<dyn Shape> = {
        let progress = animation_progress.as_shared_state();
        remember_keyed((use_full_screen_shape, ShapeKey(shape.clone())), move || {
            if *shape == *default_input_field_shape {
                // The shape can only be animated if it's the default spec value
                Arc::new(SearchBarAnimatedShape { density, progress }) as Arc<dyn Shape>
            } else if use_full_screen_shape {
                default_full_screen_shape
            } else {
                shape
            }
        })
    };
    let surface: Rc<dyn Fn()> = Rc::new(move || {
        Surface(
            shape = animated_shape.clone(),
            color = colors.container_color,
            content_color = Some(content_color_for(colors.container_color)),
            tonal_elevation = tonal_elevation,
            shadow_elevation = shadow_elevation,
            content = || {},
        );
    });

    let show_content = {
        let progress = animation_progress.clone();
        remember(move || derived_state_of(move || progress.value() > 0.0))
    }
    .get();
    let wrapped_content: Option<Rc<dyn Fn()>> = if show_content {
        let progress = animation_progress.clone();
        Some(Rc::new(move || {
            let (progress, content) = (progress.clone(), content.clone());
            Column(
                modifier = Modifier.graphics_layer_block(move |l: &mut GraphicsLayerScope| l.alpha = progress.value()),
                content = move || {
                    HorizontalDivider(color = colors.divider_color);
                    KompozitionLocalProvider(
                        &[LocalContentColor.provides(content_color_for(colors.container_color))],
                        &*content,
                    );
                },
            );
        }))
    } else {
        None
    };

    SearchBarLayout(
        animation_progress,
        final_back_progress,
        first_back_event,
        current_back_event,
        modifier,
        window_insets,
        input_field,
        surface,
        wrapped_content,
    );
}

/// [`SearchBarImpl`]'s shape while it is the default (Compose's `GenericShape` of a round rect): its corners half
/// the input field's height, less as the bar expands.
struct SearchBarAnimatedShape {
    density: Density,
    progress: SharedState<f32>,
}

impl PartialEq for SearchBarAnimatedShape {
    fn eq(&self, other: &Self) -> bool {
        // (Compose's GenericShape is equal only to itself)
        std::ptr::eq(self, other)
    }
}

impl fmt::Debug for SearchBarAnimatedShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("GenericShape")
    }
}

impl Shape for SearchBarAnimatedShape {
    fn create_outline(&self, size: Size, _layout_direction: LayoutDirection, _density: &Density) -> Outline {
        let radius = self.density.to_px(SearchBarCornerRadius * (1.0 - self.progress.get()));
        let mut path = Path::new();
        path.add_round_rect(
            &RoundRect::from_rect_radius(size.to_rect(), CornerRadius::new(radius, radius)),
            PathDirection::CounterClockwise,
        );
        Outline::Generic(path)
    }
}

/// A shape held as a key, equal by the shapes' equality.
#[derive(Clone)]
struct ShapeKey(Arc<dyn Shape>);

impl PartialEq for ShapeKey {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

/// The layout ids of the bar's parts (Compose's private `LayoutIdInputField`, `LayoutIdSurface`,
/// `LayoutIdSearchContent`).
const LayoutIdInputField: &str = "InputField";
const LayoutIdSurface: &str = "Surface";
const LayoutIdSearchContent: &str = "Content";

/// The layout of [`SearchBarImpl`] (Compose's private `SearchBarLayout`): the input field at its width (at least
/// what it is given as the bar expands) under the top insets and padding, the surface behind it growing to all it is
/// given, the content under the field; a back gesture shrinks it to nine tenths and moves it toward the gesture's
/// side and finger. The insets are paddings measured here (Compose's `windowInsetsPadding` doesn't animate).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn SearchBarLayout(
    animation_progress: Animatable<f32, AnimationVector1D>,
    final_back_progress: MutableState<f32>,
    first_back_event: MutableState<Option<BackEventCompat>>,
    current_back_event: MutableState<Option<BackEventCompat>>,
    modifier: Modifier,
    window_insets: WindowInsets,
    input_field: SearchBarSlot,
    surface: Rc<dyn Fn()>,
    content: Option<Rc<dyn Fn()>>,
) {
    // `Modifier.windowInsetsPadding` does not support animation, so the insets are converted to paddings in the
    // Layout's MeasureScope and the animation calculations are done manually.
    let unconsumed_insets = remember(MutableWindowInsets::default);
    let consumed_listener = (unconsumed_insets.clone(), window_insets.clone());
    Layout(
        modifier = modifier
            .z_index(1.0)
            .on_consumed_window_insets_changed(move |consumed_insets: &WindowInsets| {
                consumed_listener.0.set_insets(consumed_listener.1.exclude(consumed_insets));
            })
            .consume_window_insets(window_insets),
        content = move || {
            let surface = surface.clone();
            Box(
                modifier = Modifier.layout_id(LayoutIdSurface),
                propagate_min_constraints = true,
                content = move || surface(),
            );
            let input_field = input_field.clone();
            Box(
                modifier = Modifier.layout_id(LayoutIdInputField),
                propagate_min_constraints = true,
                content = move || input_field(),
            );
            if let Some(content) = content.clone() {
                Box(
                    modifier = Modifier.layout_id(LayoutIdSearchContent),
                    propagate_min_constraints = true,
                    content = move || content(),
                );
            }
        },
        measure_policy =
            measure_policy(move |scope: &MeasureScope, measurables: &[&dyn Measurable], constraints: Constraints| {
                let animation_progress = animation_progress.value();

                let input_field_measurable = find_by_id(measurables, LayoutIdInputField).expect("the input field");
                let surface_measurable = find_by_id(measurables, LayoutIdSurface).expect("the surface");
                let content_measurable = find_by_id(measurables, LayoutIdSearchContent);

                let top_padding =
                    unconsumed_insets.insets().get_top(&scope.density()) + scope.round_to_px(SearchBarVerticalPadding);
                let bottom_padding = scope.round_to_px(SearchBarVerticalPadding);

                let default_start_width =
                    constraints.constrain_width(input_field_measurable.max_intrinsic_width(constraints.max_height));
                let default_start_height =
                    constraints.constrain_height(input_field_measurable.min_intrinsic_height(constraints.max_width));

                let predictive_back_start_width =
                    (constraints.max_width as f32 * SearchBarPredictiveBackMinScale).round() as i32;
                let predictive_back_start_height =
                    (constraints.max_height as f32 * SearchBarPredictiveBackMinScale).round() as i32;
                let predictive_back_multiplier = calculate_predictive_back_multiplier(
                    current_back_event.get(),
                    animation_progress,
                    final_back_progress.get(),
                );

                let start_width =
                    lerp_int(default_start_width, predictive_back_start_width, predictive_back_multiplier);
                let start_height = lerp_int(
                    top_padding + default_start_height,
                    predictive_back_start_height,
                    predictive_back_multiplier,
                );

                let max_width = constraints.max_width;
                let max_height = constraints.max_height;

                let min_width = lerp_int(start_width, max_width, animation_progress);
                let height = lerp_int(start_height, max_height, animation_progress);

                // Note: animatedTopPadding decreases w.r.t. animationProgress
                let animated_top_padding = lerp_int(top_padding, 0, animation_progress);
                let animated_bottom_padding = lerp_int(0, bottom_padding, animation_progress);

                let input_field_placeable = input_field_measurable.measure(Constraints::new(
                    min_width,
                    max_width,
                    default_start_height,
                    default_start_height,
                ));
                let width = input_field_placeable.width();

                // As the animation proceeds, the surface loses its padding and expands to cover the entire container.
                let surface_placeable =
                    surface_measurable.measure(Constraints::fixed(width, height - animated_top_padding));
                let content_placeable = content_measurable.map(|m| {
                    m.measure(Constraints::new(
                        width,
                        width,
                        0,
                        if constraints.has_bounded_height() {
                            (constraints.max_height - (top_padding + default_start_height + bottom_padding)).max(0)
                        } else {
                            constraints.max_height
                        },
                    ))
                });

                let min_offset_margin = scope.round_to_px(SearchBarPredictiveBackMinMargin);
                let max_offset_y = scope.round_to_px(SearchBarPredictiveBackMaxOffsetY);
                let layout_direction = scope.layout_direction();
                let (current, first) = (current_back_event.get(), first_back_event.get());
                scope.layout(width, height, move |p| {
                    let predictive_back_offset_x = calculate_predictive_back_offset_x(
                        constraints,
                        min_offset_margin,
                        current,
                        layout_direction,
                        animation_progress,
                        predictive_back_multiplier,
                    );
                    let predictive_back_offset_y = calculate_predictive_back_offset_y(
                        constraints,
                        min_offset_margin,
                        current,
                        first,
                        height,
                        max_offset_y,
                        predictive_back_multiplier,
                    );

                    named_args!(p.place_relative(
                        &surface_placeable,
                        predictive_back_offset_x,
                        predictive_back_offset_y + animated_top_padding,
                    ));
                    named_args!(p.place_relative(
                        &input_field_placeable,
                        predictive_back_offset_x,
                        predictive_back_offset_y + top_padding,
                    ));
                    if let Some(content_placeable) = &content_placeable {
                        named_args!(p.place_relative(
                            content_placeable,
                            predictive_back_offset_x,
                            predictive_back_offset_y
                                + top_padding
                                + input_field_placeable.height()
                                + animated_bottom_padding,
                        ));
                    }
                })
            }),
    );
}

/// The measurable of a layout id (Kotlin's `measurables.fastFirst { it.layoutId == id }`).
fn find_by_id<'a>(measurables: &[&'a dyn Measurable], id: &'static str) -> Option<&'a dyn Measurable> {
    measurables.iter().copied().find(|m| m.layout_id().is_some_and(|i| i.is(&id)))
}

/// Kotlin's `lerp(start: Int, stop: Int, fraction: Float)` (ui-util): rounded.
fn lerp_int(start: i32, stop: i32, fraction: f32) -> i32 {
    start + ((stop - start) as f64 * fraction as f64).round() as i32
}

/// The docked forms' layout (Compose's private `DockedSearchBarLayout`): with a drop-down shape, the input field in
/// `search_bar_shape` on the container color and the content in a drop-down of `dropdown_shape` `dropdown_gap_size`
/// under it, sliding in from half its height (by the state's specs where they are springs, tweens or snaps; else
/// at once) and fading by its content animation, on a transparent rectangle; without, the field and a divider
/// and the content on a surface of `search_bar_shape`.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn DockedSearchBarLayout(
    state: SearchBarState,
    input_field: SearchBarSlot,
    modifier: Modifier,
    search_bar_shape: Arc<dyn Shape>,
    dropdown_shape: Option<Arc<dyn Shape>>,
    dropdown_gap_size: Option<Dp>,
    colors: SearchBarColors,
    tonal_elevation: Dp,
    shadow_elevation: Dp,
    content: Rc<dyn Fn()>,
) {
    let has_dropdown = dropdown_shape.is_some();
    let field_shape = search_bar_shape.clone();
    let content_state = state.clone();
    DockedSearchBarLayoutImpl(
        if has_dropdown { Arc::new(RectangleShape) } else { search_bar_shape },
        state,
        Rc::new(move || {
            if has_dropdown {
                let input_field = input_field.clone();
                Box(
                    modifier = Modifier
                        .background(colors.container_color, shape = field_shape.clone())
                        .clip(ClipShape(field_shape.clone())),
                    content = move || input_field(),
                );
            } else {
                input_field();
            }
        }),
        modifier,
        if has_dropdown { named_args!(colors.copy(container_color = Color::Transparent)) } else { colors },
        tonal_elevation,
        shadow_elevation,
        dropdown_gap_size.is_some(),
        Rc::new(move || {
            if let Some(dropdown_shape) = dropdown_shape.clone() {
                let state = content_state.clone();
                let slide_in_spec = cast_to_int_offset_spec(&*state.animation_spec_for_expand());
                let slide_out_spec = cast_to_int_offset_spec(&*state.animation_spec_for_collapse());
                let content = content.clone();
                AnimatedVisibility(
                    modifier = Modifier
                        .padding_sides(top = dropdown_gap_size.unwrap_or(Dp(0.0)))
                        .background(colors.container_color, shape = dropdown_shape.clone())
                        .clip(ClipShape(dropdown_shape))
                        .alpha(state.content_progress()),
                    visible = state.progress() > 0.1 && state.target_value() == SearchBarValue::Expanded,
                    enter = slide_in(slide_in_spec, |it: IntSize| {
                        IntOffset::new(0, (-it.height as f32 / 2.0).round() as i32)
                    }),
                    exit = slide_out(slide_out_spec, |it: IntSize| {
                        IntOffset::new(0, (-it.height as f32 / 2.0).round() as i32)
                    }),
                    content = move |_: &AnimatedVisibilityScope| {
                        let content = content.clone();
                        Column(content = move || content());
                    },
                );
            } else {
                let content = content.clone();
                Column(
                    content = move || {
                        HorizontalDivider(color = colors.divider_color);
                        content();
                    },
                );
            }
        }),
    );
}

/// Kotlin's `state.animationSpecForExpand as? FiniteAnimationSpec<IntOffset> ?: snap()`: Kotlin's cast is
/// unchecked (generics are erased), so a spring, tween or snap of floats animates offsets as it is — here the same
/// spec of offsets (a float spring's visibility threshold, meaningless for offsets, left out) —, and a spec that
/// isn't finite snaps. (A finite spec of float values — keyframes — would throw in Kotlin as it ran; it snaps here.)
fn cast_to_int_offset_spec(spec: &dyn AnimationSpec<f32>) -> Arc<dyn FiniteAnimationSpec<IntOffset>> {
    fn of<T: 'static>(any: &dyn Any) -> Option<Arc<dyn FiniteAnimationSpec<IntOffset>>> {
        if let Some(s) = any.downcast_ref::<SpringSpec<T>>() {
            return Some(Arc::new(SpringSpec::<IntOffset>::new(s.damping_ratio, s.stiffness, None)));
        }
        if let Some(s) = any.downcast_ref::<TweenSpec<T>>() {
            return Some(Arc::new(TweenSpec::<IntOffset>::new(s.duration_millis, s.delay, s.easing.clone())));
        }
        if let Some(s) = any.downcast_ref::<SnapSpec<T>>() {
            return Some(Arc::new(SnapSpec::<IntOffset>::new(s.delay)));
        }
        None
    }
    let any = spec.as_any();
    of::<f32>(any).or_else(|| of::<()>(any)).unwrap_or_else(|| Arc::new(snap::<IntOffset>()))
}

/// What both docked forms' layouts are made of (Compose's private `DockedSearchBarLayoutImpl`): back collapses the
/// state; on a [`Surface`] of `shape` (padded by the software keyboard), the input field and the content under it,
/// as wide as the collapsed bar, as tall as they are within the collapsed bar's height and a share of the window's
/// (two thirds; a half with a gap) at least 240 dp, growing with the state's animation.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn DockedSearchBarLayoutImpl(
    shape: Arc<dyn Shape>,
    state: SearchBarState,
    input_field: SearchBarSlot,
    modifier: Modifier,
    colors: SearchBarColors,
    tonal_elevation: Dp,
    shadow_elevation: Dp,
    has_gap: bool,
    content: Rc<dyn Fn()>,
) {
    let scope = remember_coroutine_scope();
    {
        let state = state.clone();
        BackHandler(enabled = state.is_expanded(), on_back = move || launch_animate_to_collapsed(&scope, &state));
    }

    Surface(
        shape = shape,
        color = colors.container_color,
        content_color = Some(content_color_for(colors.container_color)),
        tonal_elevation = tonal_elevation,
        shadow_elevation = shadow_elevation,
        modifier = modifier.ime_padding(),
        content = move || {
            let window_container_height = get_window_container_height();
            let max_height_screen_ratio = if has_gap {
                DockedExpandedWithGapTableMaxHeightScreenRatio
            } else {
                DockedExpandedTableMaxHeightScreenRatio
            };
            let max_height = window_container_height * max_height_screen_ratio;
            let min_height = DockedExpandedTableMinHeight.coerce_at_most(max_height);

            let (input_field, content) = (input_field.clone(), content.clone());
            MultiContentLayout(
                contents =
                    vec![Rc::new(move || input_field()) as Rc<dyn Fn()>, Rc::new(move || content()) as Rc<dyn Fn()>],
                measure_policy =
                    Rc::new(DockedMeasurePolicy { state, max_height, min_height }) as Rc<dyn MultiContentMeasurePolicy>,
            );
        },
    );
}

/// [`DockedSearchBarLayoutImpl`]'s measure policy (Compose's `Layout(contents = listOf(inputField, content))`
/// block).
struct DockedMeasurePolicy {
    state: SearchBarState,
    max_height: Dp,
    min_height: Dp,
}

impl MultiContentMeasurePolicy for DockedMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        base_constraints: Constraints,
    ) -> MeasureResult {
        let (input_field_measurables, content_measurables) = (&measurables[0], &measurables[1]);
        let constraint_max_height = lerp_int(
            self.state.collapsed_bounds().height(),
            scope.round_to_px(self.max_height),
            self.state.animatable_value().max(0.0),
        );
        let constraints = base_constraints.constrain(Constraints::new(
            0,
            self.state.collapsed_coords().map_or(base_constraints.max_width, |it| it.size().width),
            scope.round_to_px(self.min_height).min(constraint_max_height),
            constraint_max_height,
        ));
        let loose_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));

        let input_field_placeables: Vec<_> =
            input_field_measurables.iter().map(|it| it.measure(loose_constraints)).collect();
        let input_field_width = input_field_placeables.iter().map(|it| it.width()).max().unwrap_or(0);
        let input_field_height = input_field_placeables.iter().map(|it| it.height()).max().unwrap_or(0);

        let content_constraints = named_args!(
            Constraints::offset(&loose_constraints, 0, -input_field_height).copy(max_width = input_field_width)
        );
        let content_placeables: Vec<_> = content_measurables.iter().map(|it| it.measure(content_constraints)).collect();

        let height = input_field_height + content_placeables.iter().map(|it| it.height()).max().unwrap_or(0);
        let width = input_field_width.max(content_placeables.iter().map(|it| it.width()).max().unwrap_or(0));

        scope.layout(constraints.constrain_width(width), constraints.constrain_height(height), move |p| {
            for it in &input_field_placeables {
                named_args!(p.place(it, 0, 0));
            }
            for it in &content_placeables {
                named_args!(p.place(it, 0, input_field_height));
            }
        })
    }
}

/// The full-screen forms' layout (Compose's private `FullScreenSearchBarLayout`): the surface growing from the
/// collapsed bar's bounds to all it is given (contained: at that size at once, fading in), the input field moving
/// from the bar's place to the top (padded by the insets' top, animated, and `input_field_padding` at the sides),
/// the content under it fading in; a back gesture shrinks it to nine tenths (never under the collapsed bar) toward
/// the gesture's side and finger, its corners rounding.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn FullScreenSearchBarLayout(
    state: SearchBarState,
    predictive_back_state: PredictiveBackState,
    input_field: SearchBarSlot,
    input_field_padding: PaddingValues,
    modifier: Modifier,
    collapsed_shape: Arc<dyn Shape>,
    colors: SearchBarColors,
    tonal_elevation: Dp,
    shadow_elevation: Dp,
    window_insets: WindowInsets,
    is_contained: bool,
    content: Rc<dyn Fn()>,
) {
    let back_event = {
        let predictive_back_state = predictive_back_state.clone();
        remember(move || derived_state_of(move || predictive_back_state.value()))
    }
    .get();
    let first_in_progress_value = remember(|| mutable_state_of(None::<InProgress>));
    match back_event {
        BackEventProgress::InProgress { .. } => {
            if first_in_progress_value.peek().is_none() {
                first_in_progress_value.set(InProgress::of(back_event));
            }
        }
        BackEventProgress::NotRunning => first_in_progress_value.set(None),
        BackEventProgress::Completed => {}
    }
    let last_in_progress_value = remember(|| mutable_state_of(None::<InProgress>));
    match back_event {
        BackEventProgress::InProgress { .. } => last_in_progress_value.set(InProgress::of(back_event)),
        BackEventProgress::NotRunning => last_in_progress_value.set(None),
        BackEventProgress::Completed => {}
    }

    let density = LocalDensity.current();
    let full_screen_shape = SearchBarDefaults::full_screen_shape();
    let animated_shape: Arc<dyn Shape> = {
        let (progress, last_in_progress_value, collapsed_shape, full_screen_shape) = (
            state.shared_animatable_value(),
            last_in_progress_value.clone(),
            collapsed_shape.clone(),
            full_screen_shape.clone(),
        );
        remember_keyed(
            (density.density.to_bits(), density.font_scale.to_bits(), ShapeKey(full_screen_shape.clone())),
            move || {
                Arc::new(FullScreenAnimatedShape {
                    density,
                    progress,
                    last_in_progress_value,
                    collapsed_shape,
                    full_screen_shape,
                }) as Arc<dyn Shape>
            },
        )
    };

    // Top window insets need to be animated, but `Modifier.windowInsetsPadding` does not support animation. The top
    // insets are separated out so the animation calculations can be done manually in the Layout's MeasureScope.
    let unconsumed_insets = remember(MutableWindowInsets::default);
    let non_top_insets = unconsumed_insets.insets().only(WindowInsetsSides::Horizontal + WindowInsetsSides::Bottom);
    let consumed_listener = (unconsumed_insets.clone(), window_insets.clone());
    let measure_state = state.clone();
    let (first_in, last_in) = (first_in_progress_value.clone(), last_in_progress_value.clone());
    Layout(
        modifier = modifier
            .on_consumed_window_insets_changed(move |consumed_insets: &WindowInsets| {
                consumed_listener.0.set_insets(consumed_listener.1.exclude(consumed_insets));
            })
            .consume_window_insets(window_insets),
        content = move || {
            let input_field = input_field.clone();
            Box(
                modifier = Modifier
                    .layout_id(LayoutIdInputField)
                    .padding_values(non_top_insets.only(WindowInsetsSides::Horizontal).as_padding_values())
                    .clip(ClipShape(collapsed_shape.clone())),
                propagate_min_constraints = true,
                content = move || input_field(),
            );

            Surface(
                modifier = Modifier.layout_id(LayoutIdSurface),
                shape = animated_shape.clone(),
                color = colors.container_color,
                content_color = Some(content_color_for(colors.container_color)),
                tonal_elevation = tonal_elevation,
                shadow_elevation = shadow_elevation,
                content = || {},
            );

            let content = content.clone();
            Column(
                modifier = Modifier.layout_id(LayoutIdSearchContent).padding_values(non_top_insets.as_padding_values()),
                content = move || content(),
            );
        },
        measure_policy =
            measure_policy(move |scope: &MeasureScope, measurables: &[&dyn Measurable], constraints: Constraints| {
                let state = &measure_state;
                let last_in_progress_value = last_in.get();
                let predictive_back_progress = transform(last_in_progress_value);
                let collapsed_bounds = state.collapsed_bounds();
                let collapsed_width = Some(collapsed_bounds.width())
                    .filter(|it| *it != 0)
                    .unwrap_or_else(|| scope.round_to_px(SearchBarMinWidth));
                let collapsed_height = Some(collapsed_bounds.height())
                    .filter(|it| *it != 0)
                    .unwrap_or_else(|| scope.round_to_px(InputFieldHeight));

                let predictive_back_end_width = ((constraints.max_width as f32 * SearchBarPredictiveBackMinScale)
                    .round() as i32)
                    .max(collapsed_width);
                let predictive_back_end_height = ((constraints.max_height as f32 * SearchBarPredictiveBackMinScale)
                    .round() as i32)
                    .max(collapsed_height);
                let end_width = lerp_int(constraints.max_width, predictive_back_end_width, predictive_back_progress);
                let end_height = lerp_int(constraints.max_height, predictive_back_end_height, predictive_back_progress);
                let progress = state.progress();
                let (width, height) = if is_contained {
                    (end_width, end_height)
                } else {
                    (
                        constraints.constrain_width(lerp_int(collapsed_width, end_width, progress)),
                        constraints.constrain_height(lerp_int(collapsed_height, end_height, progress)),
                    )
                };

                let surface_measurable = find_by_id(measurables, LayoutIdSurface).expect("the surface");
                let surface_placeable = surface_measurable.measure(Constraints::fixed(width, height));

                let layout_direction = scope.layout_direction();
                let start_padding = scope.round_to_px(input_field_padding.calculate_start_padding(layout_direction));
                let end_padding = scope.round_to_px(input_field_padding.calculate_end_padding(layout_direction));
                let padded_input_field_width =
                    lerp_int(collapsed_width, width - start_padding - end_padding, state.animatable_value());
                let input_field_measurable = find_by_id(measurables, LayoutIdInputField).expect("the input field");
                let input_field_placeable =
                    input_field_measurable.measure(Constraints::fixed(padded_input_field_width, collapsed_height));

                let top_padding = unconsumed_insets.insets().get_top(&scope.density())
                    + if is_contained {
                        scope.round_to_px(AppBarWithSearchVerticalPadding)
                    } else {
                        scope.round_to_px(SearchBarVerticalPadding)
                    };
                let animated_top_padding = lerp_int(0, top_padding, progress.min(1.0 - predictive_back_progress));
                let bottom_padding = if is_contained {
                    scope.round_to_px(SearchBarVerticalPadding)
                } else {
                    lerp_int(0, scope.round_to_px(SearchBarVerticalPadding), progress)
                };

                let padded_input_field_height = input_field_placeable.height() + animated_top_padding + bottom_padding;
                let content_measurable = find_by_id(measurables, LayoutIdSearchContent).expect("the content");
                let content_placeable = content_measurable.measure(Constraints::new(
                    width,
                    width,
                    0,
                    (height - padded_input_field_height).max(0),
                ));

                let min_margin = scope.round_to_px(SearchBarPredictiveBackMinMargin);
                let max_offset_y_px = scope.round_to_px(SearchBarPredictiveBackMaxOffsetY);
                let first_in_progress_value = first_in.get();
                let state = state.clone();
                scope.layout(constraints.max_width, constraints.max_height, move |p| {
                    let end_offset_x = |it: &InProgress| -> i32 {
                        let x = if it.swipe_edge == ProgressSwipeEdge::Left {
                            constraints.max_width - min_margin - predictive_back_end_width
                        } else {
                            min_margin
                        };
                        x.max(collapsed_bounds.right - predictive_back_end_width).min(collapsed_bounds.left)
                    };

                    let end_offset_y = |it: &InProgress| -> i32 {
                        let Some(first) = &first_in_progress_value else { return 0 };
                        let absolute_delta_y = it.touch_y - first.touch_y;
                        let relative_delta_y = absolute_delta_y.abs() / constraints.max_height as f32;

                        let available_vertical_space =
                            ((constraints.max_height - predictive_back_end_height) / 2 - min_margin).max(0);
                        let total_offset_y = available_vertical_space.min(max_offset_y_px);
                        let interpolated_offset_y = lerp_int(0, total_offset_y, relative_delta_y);
                        (interpolated_offset_y * kt_sign(absolute_delta_y) as i32 + top_padding)
                            .min(collapsed_bounds.top)
                    };

                    let predictive_back_offset_x =
                        lerp_int(0, last_in_progress_value.as_ref().map_or(0, end_offset_x), predictive_back_progress);
                    let offset_x = if is_contained {
                        predictive_back_offset_x
                    } else {
                        lerp_int(collapsed_bounds.left, predictive_back_offset_x, progress)
                    };
                    let current_center_x = lerp_f32(
                        collapsed_bounds.center().x as f32,
                        offset_x as f32 + width as f32 / 2.0,
                        state.animatable_value(),
                    );
                    let offset_y =
                        lerp_int(0, last_in_progress_value.as_ref().map_or(0, end_offset_y), predictive_back_progress);
                    let animated_offset_y = lerp_int(collapsed_bounds.top, offset_y, progress);

                    let surface_state = state.clone();
                    p.place_with_layer(
                        &surface_placeable,
                        offset_x,
                        if is_contained { offset_y } else { animated_offset_y },
                        0.0,
                        move |l: &mut GraphicsLayerScope| {
                            if is_contained {
                                l.alpha = surface_state.progress();
                            }
                        },
                    );
                    named_args!(p.place(
                        &input_field_placeable,
                        (current_center_x - input_field_placeable.width() as f32 / 2.0).round() as i32,
                        animated_offset_y + animated_top_padding,
                    ));
                    let content_state = state.clone();
                    p.place_with_layer(
                        &content_placeable,
                        offset_x,
                        animated_offset_y + animated_top_padding + input_field_placeable.height() + bottom_padding,
                        0.0,
                        move |l: &mut GraphicsLayerScope| {
                            l.alpha =
                                if is_contained { content_state.content_progress() } else { content_state.progress() };
                        },
                    );
                })
            }),
    );
}

/// A predictive back in progress (Compose's `BackEventProgress.InProgress`): what the full-screen layout keeps of
/// the first and last steps.
#[derive(Clone, Copy, PartialEq, Debug)]
struct InProgress {
    touch_y: f32,
    progress: f32,
    swipe_edge: ProgressSwipeEdge,
}

impl InProgress {
    fn of(event: BackEventProgress) -> Option<InProgress> {
        match event {
            BackEventProgress::InProgress { touch_y, progress, swipe_edge, .. } => {
                Some(InProgress { touch_y, progress, swipe_edge })
            }
            _ => None,
        }
    }
}

/// Compose's private `BackEventProgress.InProgress?.transform()`: the progress through the predictive back easing,
/// 0 without one.
fn transform(event: Option<InProgress>) -> f32 {
    event.map_or(0.0, |it| PredictiveBack::transform(it.progress))
}

/// Kotlin's `sign` of a float: -1, 0 or 1 (NaN for NaN).
fn kt_sign(value: f32) -> f32 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        value
    }
}

/// Kotlin's `lerp(start: Float, stop: Float, fraction: Float)` (ui-util).
fn lerp_f32(start: f32, stop: f32, fraction: f32) -> f32 {
    (1.0 - fraction) * start + fraction * stop
}

/// The full-screen layout's surface shape (Compose's `GenericShape` of `FullScreenSearchBarLayout`): from the
/// collapsed shape to the full-screen one — while they are the defaults (a circle and a rectangle), a round rect
/// whose corners shrink as the state expands and grow back with a back gesture; else the collapsed shape under
/// half way, the full-screen one past it.
struct FullScreenAnimatedShape {
    density: Density,
    progress: SharedState<f32>,
    last_in_progress_value: MutableState<Option<InProgress>>,
    collapsed_shape: Arc<dyn Shape>,
    full_screen_shape: Arc<dyn Shape>,
}

impl PartialEq for FullScreenAnimatedShape {
    fn eq(&self, other: &Self) -> bool {
        // (Compose's GenericShape is equal only to itself)
        std::ptr::eq(self, other)
    }
}

impl fmt::Debug for FullScreenAnimatedShape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("GenericShape")
    }
}

impl Shape for FullScreenAnimatedShape {
    fn create_outline(&self, size: Size, layout_direction: LayoutDirection, _density: &Density) -> Outline {
        let mut path = Path::new();
        let progress = self.progress.get().clamp(0.0, 1.0);
        // (Kotlin's `collapsedShape === CircleShape && fullScreenShape === RectangleShape`: the defaults by
        // identity — here by equality, KompozR's shapes being values)
        if self.collapsed_shape == CircleShape && self.full_screen_shape == RectangleShape {
            // The shape can only be animated if it's the default spec value
            let fraction = (1.0 - progress).max(transform(self.last_in_progress_value.get()));
            let radius = self.density.to_px(SearchBarCornerRadius * fraction);
            if radius < 1e-3 {
                path.add_rect(size.to_rect(), PathDirection::CounterClockwise);
            } else {
                path.add_round_rect(
                    &RoundRect::from_rect_radius(size.to_rect(), CornerRadius::new(radius, radius)),
                    PathDirection::CounterClockwise,
                );
            }
        } else {
            let shape = if progress < 0.5 { &self.collapsed_shape } else { &self.full_screen_shape };
            path.add_outline(&shape.create_outline(size, layout_direction, &self.density));
        }
        Outline::Generic(path)
    }
}

/// Keeps a text field in `content` from showing the software keyboard (Compose's private `DisableSoftKeyboard`):
/// its input method requests are intercepted, never passed on.
#[kompozable]
fn DisableSoftKeyboard(content: impl Fn() + 'static) {
    let interceptor: Rc<dyn PlatformTextInputInterceptor> = remember(|| {
        Rc::new(|_, _| Box::pin(await_cancellation()) as kompoz_ui::platform_text_input::SessionFuture)
            as Rc<dyn PlatformTextInputInterceptor>
    });
    InterceptPlatformTextInput(interceptor, content);
}

/// Calls `on_click` each time `interaction_source` releases a press (Compose's private
/// `DetectClickFromInteractionSource`).
#[kompozable]
pub(crate) fn DetectClickFromInteractionSource(interaction_source: InteractionSource, on_click: impl Fn() + 'static) {
    let on_click = Rc::new(on_click);
    let source = interaction_source.clone();
    LaunchedEffect(interaction_source, async move {
        source
            .interactions()
            .collect(move |interaction| {
                if interaction.is::<PressInteraction::Release>() {
                    on_click();
                }
            })
            .await;
    });
}

/// How much of a back gesture the layout follows (Compose's private `calculatePredictiveBackMultiplier`).
fn calculate_predictive_back_multiplier(
    current_back_event: Option<BackEventCompat>,
    progress: f32,
    final_back_progress: f32,
) -> f32 {
    if current_back_event.is_none() {
        0.0 // Not in predictive back at all.
    } else if final_back_progress.is_nan() {
        1.0 // User is currently swiping predictive back.
    } else if final_back_progress <= 0.0 {
        0.0 // Safety check for divide by zero.
    } else {
        progress / final_back_progress // User has released predictive back swipe.
    }
}

/// How far a back gesture moves the bar sideways (Compose's private `calculatePredictiveBackOffsetX`).
fn calculate_predictive_back_offset_x(
    constraints: Constraints,
    min_margin: i32,
    current_back_event: Option<BackEventCompat>,
    layout_direction: LayoutDirection,
    progress: f32,
    predictive_back_multiplier: f32,
) -> i32 {
    let Some(current_back_event) = current_back_event else { return 0 };
    if predictive_back_multiplier == 0.0 {
        return 0;
    }
    let direction_multiplier = if current_back_event.swipe_edge == SwipeEdge::Left { 1.0 } else { -1.0 };
    let rtl_multiplier = if layout_direction == LayoutDirection::Ltr { 1.0 } else { -1.0 };
    let max_offset_x = (constraints.max_width as f32 * SearchBarPredictiveBackMaxOffsetXRatio) - min_margin as f32;
    let interpolated_offset_x = max_offset_x * (1.0 - progress);
    (interpolated_offset_x * predictive_back_multiplier * direction_multiplier * rtl_multiplier).round() as i32
}

/// How far a back gesture moves the bar with the finger (Compose's private `calculatePredictiveBackOffsetY`).
fn calculate_predictive_back_offset_y(
    constraints: Constraints,
    min_margin: i32,
    current_back_event: Option<BackEventCompat>,
    first_back_event: Option<BackEventCompat>,
    height: i32,
    max_offset_y: i32,
    predictive_back_multiplier: f32,
) -> i32 {
    let (Some(first_back_event), Some(current_back_event)) = (first_back_event, current_back_event) else {
        return 0;
    };
    if predictive_back_multiplier == 0.0 {
        return 0;
    }
    let available_vertical_space = ((constraints.max_height - height) / 2 - min_margin).max(0);
    let adjusted_max_offset_y = available_vertical_space.min(max_offset_y);
    let y_delta = current_back_event.touch_y - first_back_event.touch_y;
    let y_progress = y_delta.abs() / constraints.max_height as f32;
    let direction_multiplier = kt_sign(y_delta);
    let interpolated_offset_y = lerp_int(0, adjusted_max_offset_y, y_progress);
    (interpolated_offset_y as f32 * predictive_back_multiplier * direction_multiplier).round() as i32
}

// ---------------------------------------------------------------------------------------------------------------
// Measurement and animation specs

/// The padding of a search bar as a top bar (Compose's internal `SearchBarAsTopBarPadding`).
pub(crate) const SearchBarAsTopBarPadding: Dp = Dp(8.0);
const AppBarWithSearchHorizontalPadding: Dp = Dp(4.0);
/// The padding above and below the search bar of an app bar (Compose's internal
/// `AppBarWithSearchVerticalPadding`).
pub(crate) const AppBarWithSearchVerticalPadding: Dp = Dp(4.0);

const SearchBarCornerRadius: Dp = Dp(InputFieldHeight.0 / 2.0);
const FullScreenExpandedHorizontalPadding: Dp = Dp(8.0);
/// The least height of a docked bar's table of results (Compose's internal `DockedExpandedTableMinHeight`).
pub(crate) const DockedExpandedTableMinHeight: Dp = Dp(240.0);
const DockedExpandedTableMaxHeightScreenRatio: f32 = 2.0 / 3.0;
const DockedExpandedWithGapTableMaxHeightScreenRatio: f32 = 1.0 / 2.0;
/// A search bar's least and largest widths and its padding above and below (Compose's internal
/// `SearchBarMinWidth`, `SearchBarMaxWidth`, `SearchBarVerticalPadding`).
pub(crate) const SearchBarMinWidth: Dp = Dp(360.0);
pub(crate) const SearchBarMaxWidth: Dp = Dp(720.0);
pub(crate) const SearchBarVerticalPadding: Dp = Dp(8.0);

// Search bar has 16dp padding between icons and start/end, while by default text field has 12dp.
/// How far a search bar's icons are moved in (Compose's private `SearchBarIconOffsetX`).
pub(crate) const SearchBarIconOffsetX: Dp = Dp(4.0);
const SearchBarPredictiveBackMinScale: f32 = 9.0 / 10.0;
const SearchBarPredictiveBackMinMargin: Dp = Dp(8.0);
const SearchBarPredictiveBackMaxOffsetXRatio: f32 = 1.0 / 20.0;
const SearchBarPredictiveBackMaxOffsetY: Dp = Dp(24.0);

// Animation specs
const AnimationEnterDurationMillis: i32 = MotionTokens::DurationLong4 as i32;
const AnimationExitDurationMillis: i32 = MotionTokens::DurationMedium3 as i32;
/// The delay of a search bar's animations (Compose's private `AnimationDelayMillis`).
pub(crate) const AnimationDelayMillis: i32 = MotionTokens::DurationShort2 as i32;
fn AnimationEnterEasing() -> Arc<dyn kompoz_animation::Easing> {
    Arc::new(MotionTokens::EasingEmphasizedDecelerateCubicBezier)
}
fn AnimationExitEasing() -> Arc<dyn kompoz_animation::Easing> {
    Arc::new(CubicBezierEasing::constant(0.0, 1.0, 0.0, 1.0))
}
fn AnimationEnterFloatSpec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(TweenSpec::<f32>::new(AnimationEnterDurationMillis, AnimationDelayMillis, AnimationEnterEasing()))
}
fn AnimationExitFloatSpec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(TweenSpec::<f32>::new(AnimationExitDurationMillis, AnimationDelayMillis, AnimationExitEasing()))
}
fn AnimationPredictiveBackExitFloatSpec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(TweenSpec::<f32>::new(AnimationExitDurationMillis, 0, AnimationExitEasing()))
}
fn AnimationEnterSizeSpec() -> Arc<dyn FiniteAnimationSpec<IntSize>> {
    Arc::new(TweenSpec::<IntSize>::new(AnimationEnterDurationMillis, AnimationDelayMillis, AnimationEnterEasing()))
}
fn AnimationExitSizeSpec() -> Arc<dyn FiniteAnimationSpec<IntSize>> {
    Arc::new(TweenSpec::<IntSize>::new(AnimationExitDurationMillis, AnimationDelayMillis, AnimationExitEasing()))
}
fn DockedEnterTransition() -> EnterTransition {
    named_args!(fade_in(Arc::new(TweenSpec::<f32>::new(
        AnimationEnterDurationMillis,
        AnimationDelayMillis,
        AnimationEnterEasing()
    )))) + named_args!(expand_vertically(animation_spec = AnimationEnterSizeSpec()))
}
fn DockedExitTransition() -> ExitTransition {
    named_args!(fade_out(Arc::new(TweenSpec::<f32>::new(
        AnimationExitDurationMillis,
        AnimationDelayMillis,
        AnimationExitEasing()
    )))) + named_args!(shrink_vertically(animation_spec = AnimationExitSizeSpec()))
}
fn AnimationForContentFadeInSpec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(TweenSpec::<f32>::new(
        MotionTokens::DurationShort2 as i32,
        MotionTokens::DurationShort1 as i32,
        MotionTokens::EasingStandardAccelerateCubicBezier,
    ))
}
fn AnimationForContentFadeOutSpec() -> Arc<dyn AnimationSpec<f32>> {
    Arc::new(TweenSpec::<f32>::new(
        MotionTokens::DurationShort2 as i32,
        0,
        MotionTokens::EasingStandardDecelerateCubicBezier,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::basic_edge_to_edge_dialog::remember_predictive_back_state;
    use crate::text::Text;
    use kompoz_runtime::kompozable_lambda;
    use kompoz_test::KompozTestRule;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() <= 0.5
    }

    fn bounds(rule: &KompozTestRule, tag: &str) -> kompoz_graphics::Rect {
        rule.on_node_with_tag_unmerged(tag).get_unclipped_bounds_in_root()
    }

    /// The surface drawn (the last path in the container color): its bounds and its corner's radius (where its
    /// diagonal from the top left corner enters the path: `r (1 - 1/√2)` in).
    fn surface(rule: &KompozTestRule, color: Color) -> (kompoz_graphics::Rect, f32) {
        let canvas = rule.draw();
        let i = canvas.path_colors.iter().rposition(|c| *c == color).expect("the surface drawn");
        let path = &canvas.path_shapes[i];
        let corner = path.get_bounds().top_left();
        if path.contains(corner + Offset::new(0.01, 0.01)) {
            return (canvas.paths[i], 0.0);
        }
        let (mut inside, mut outside) = (30.0f32, 0.0f32);
        for _ in 0..40 {
            let t = (inside + outside) / 2.0;
            if path.contains(corner + Offset::new(t, t)) { inside = t } else { outside = t }
        }
        (canvas.paths[i], inside / (1.0 - std::f32::consts::FRAC_1_SQRT_2))
    }

    /// The full-screen layout's predictive back (Android's: on the desktop the dialog's predictive back state never
    /// runs), checked against the C++ `full_screen_search_grows_out_of_its_bar_and_follows_the_back_gesture`'s
    /// numbers, which follow Compose's: half way back from the left, the finger 200 dp lower, the view nine tenths of
    /// the window (no narrower than the bar), to the right 8 dp from the edge (no further than the bar), 6 dp lower
    /// with the finger, the field's 8 dp above going, its corners rounding to the bar's; from the right, 8 dp in; a
    /// bar near the top and wider than nine tenths keeps the view at its width and no lower than it.
    fn full_screen_layout_follows_the_predictive_back() {
        let wide = mutable_state_of(false);
        let back_slot: Rc<RefCell<Option<PredictiveBackState>>> = Rc::new(RefCell::new(None));
        let colors_slot: Rc<RefCell<Option<Color>>> = Rc::new(RefCell::new(None));
        let rule = {
            let (wide, back_slot, colors_slot) = (wide.clone(), back_slot.clone(), colors_slot.clone());
            KompozTestRule::with_viewport(
                kompozable_lambda!(move || {
                    let state = remember(|| {
                        SearchBarState::new(
                            SearchBarValue::Expanded,
                            Arc::new(snap::<f32>()),
                            Arc::new(snap::<f32>()),
                            Arc::new(snap::<f32>()),
                            Arc::new(snap::<f32>()),
                        )
                    });
                    let back = remember_predictive_back_state();
                    *back_slot.borrow_mut() = Some(back.clone());
                    let colors = SearchBarDefaults.colors();
                    *colors_slot.borrow_mut() = Some(colors.container_color);
                    let padding = if wide.get() {
                        Modifier.padding_sides(Dp(16.0), Dp(10.0), Dp(16.0), Dp(0.0))
                    } else {
                        Modifier.padding_sides(Dp(40.0), Dp(100.0), Dp(40.0), Dp(0.0))
                    };
                    let (bar_state, layout_state) = (state.clone(), state.clone());
                    Box(
                        modifier = Modifier.fill_max_size(),
                        content = move || {
                            let bar_state = bar_state.clone();
                            Box(
                                modifier = padding.fill_max_width(),
                                content = move || {
                                    SearchBar(
                                        bar_state.clone(),
                                        input_field = || Box(modifier = Modifier.fill_max_width().height(Dp(56.0))),
                                    );
                                },
                            );
                            FullScreenSearchBarLayout(
                                layout_state.clone(),
                                back.clone(),
                                Rc::new(|| Box(modifier = Modifier.fill_max_size().test_tag("açık"))),
                                PaddingValues::all(Dp(0.0)),
                                Modifier,
                                SearchBarDefaults::input_field_shape(),
                                colors,
                                Dp(0.0),
                                Dp(0.0),
                                WindowInsets::Zero,
                                false,
                                Rc::new(|| Text("Sonuç", modifier = Modifier.test_tag("sonuç"))),
                            );
                        },
                    );
                }),
                IntSize::new(400, 800),
            )
        };
        let back = back_slot.borrow().clone().expect("composed");
        let color = colors_slot.borrow().expect("composed");
        let field = || bounds(&rule, "açık");
        let f = field();
        assert!(near(f.left, 0.0) && near(f.top, 8.0) && near(f.width(), 400.0), "{f:?}");
        let (s, radius) = surface(&rule, color);
        assert!(near(s.height(), 800.0) && radius == 0.0, "{s:?} {radius}");
        let b = PredictiveBack::transform(0.5);
        let step = |touch_x: f32, touch_y: f32, progress: f32, swipe_edge| {
            back.set_value(BackEventProgress::InProgress { touch_x, touch_y, progress, swipe_edge });
            rule.advance_frame();
        };
        let lerp = |a: f32, z: f32, t: f32| a + (z - a) * t;
        step(10.0, 400.0, 0.0, ProgressSwipeEdge::Left);
        step(60.0, 600.0, 0.5, ProgressSwipeEdge::Left);
        let f = field();
        assert!(near(f.left, 32.0 * b) && near(f.width(), lerp(400.0, 360.0, b)), "{f:?} {b}");
        // (each part rounded to whole pixels, as Compose's integer lerps are)
        assert!(near(f.top, (14.0 * b).round() + (8.0 * (1.0 - b)).round()), "{f:?}");
        let (s, radius) = surface(&rule, color);
        assert!((radius - 28.0 * b).abs() < 0.6, "{radius} {b}");
        assert!(near(s.height(), lerp(800.0, 720.0, b)) && near(s.top, lerp(0.0, 14.0, b)), "{s:?}");
        back.set_value(BackEventProgress::NotRunning); // called off: as it was
        rule.advance_frame();
        assert!(near(field().left, 0.0) && near(field().top, 8.0) && near(field().width(), 400.0));
        step(390.0, 400.0, 0.0, ProgressSwipeEdge::Right);
        step(340.0, 400.0, 0.5, ProgressSwipeEdge::Right);
        assert!(near(field().left, 8.0 * b), "{:?}", field());
        back.set_value(BackEventProgress::NotRunning);
        wide.set(true);
        rule.wait_for_idle();
        step(10.0, 400.0, 0.0, ProgressSwipeEdge::Left);
        step(60.0, 600.0, 0.5, ProgressSwipeEdge::Left);
        let k = field();
        assert!(near(k.left, 16.0 * b) && near(k.width(), lerp(400.0, 368.0, b)), "{k:?}");
        assert!(near(k.top, (10.0 * b).round() + (8.0 * (1.0 - b)).round()), "{k:?}");
    }

    /// Kotlin's unchecked cast of a state's float spec to an offset spec: a spring, tween or snap the same of
    /// offsets, anything else a snap.
    fn float_specs_cast_to_offset_specs() {
        let spring: Arc<dyn AnimationSpec<f32>> = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
        let cast = cast_to_int_offset_spec(&*spring);
        assert!(AnimationSpec::<IntOffset>::as_any(&*cast).downcast_ref::<SpringSpec<IntOffset>>().is_some());
        let tween: Arc<dyn AnimationSpec<f32>> =
            Arc::new(TweenSpec::<f32>::new(300, 50, kompoz_animation::LinearEasing));
        let cast = cast_to_int_offset_spec(&*tween);
        let t = AnimationSpec::<IntOffset>::as_any(&*cast).downcast_ref::<TweenSpec<IntOffset>>().expect("a tween");
        assert!(t.duration_millis == 300 && t.delay == 50);
        let repeating: Arc<dyn AnimationSpec<f32>> = Arc::new(kompoz_animation::InfiniteRepeatableSpec::new(
            TweenSpec::<f32>::new(300, 0, kompoz_animation::LinearEasing),
            kompoz_animation::RepeatMode::Restart,
            kompoz_animation::StartOffset::default(),
        ));
        let cast = cast_to_int_offset_spec(&*repeating);
        assert!(AnimationSpec::<IntOffset>::as_any(&*cast).downcast_ref::<SnapSpec<IntOffset>>().is_some());
    }

    #[test]
    fn search_bar_internals() {
        kompoz_runtime::set_ui_thread();
        full_screen_layout_follows_the_predictive_back();
        float_specs_cast_to_offset_specs();
    }
}
