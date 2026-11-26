//! Material's bottom sheet scaffold, as Compose's BottomSheetScaffold.kt: a screen with a standard (persistent)
//! bottom sheet peeking out below its content ([`BottomSheetScaffold`]), an optional top bar and a snackbar host,
//! and its state ([`BottomSheetScaffoldState`], [`remember_bottom_sheet_scaffold_state`],
//! [`remember_standard_bottom_sheet_state`], deprecated by material3 1.5.0-alpha20 for
//! [`remember_bottom_sheet_state`](crate::remember_bottom_sheet_state)).

// (the deprecated rememberStandardBottomSheetState is defined here)
#![allow(deprecated)]

use crate::color_scheme::content_color_for;
use crate::content_color::LocalContentColor;
use crate::internal::draggable_anchors::DraggableAnchorsModifiers;
use crate::internal::strings::{Strings, get_string};
use crate::material_theme::MaterialTheme;
use crate::sheet_defaults::{
    BottomSheetDefaults, ConsumeSwipeWithinBottomSheetBoundsNestedScrollConnection, DragHandleWithTooltip,
    SheetModifiers, SheetState, SheetValue, default_drag_handle, remember_bottom_sheet_state, remember_sheet_state,
    remembered_default_spatial_spec, remembered_positional_threshold,
};
use crate::snackbar_host::{SnackbarHost, SnackbarHostState};
use crate::surface::Surface;
use kompoz_coroutines::remember_coroutine_scope;
use kompoz_foundation::gestures::{
    AnchoredDraggableDefaults, DraggableAnchors, DraggableAnchorsConfig, FlingBehavior, Orientation,
};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Dp, Shape};
use kompoz_runtime::kt::{panic_kt, set_of};
use kompoz_runtime::{
    KompozitionLocalProvider, KotlinProperties, Memoized, SideEffect, kompozable, kompozable_block, named, named_args,
    remember, remember_keyed,
};
use kompoz_ui::input::NestedScrollConnection;
use kompoz_ui::layout::{MultiContentLayout, MultiContentMeasurePolicy};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::{Constraints, Measurable, MeasureResult, MeasureScope, Modifier};
use std::rc::Rc;
use std::sync::Arc;

use SheetValue::{Expanded, Hidden, PartiallyExpanded};

/// A screen with a standard bottom sheet (Compose's `BottomSheetScaffold`): `content` (given padding of the peek
/// height at the bottom) below `top_bar` (None: none), a sheet of `sheet_content` (on a [`Surface`] of
/// `sheet_shape`, `sheet_container_color`, `sheet_content_color` — None: the container's content color —,
/// `sheet_tonal_elevation` and `sheet_shadow_elevation`, at most `sheet_max_width` wide, centered, under
/// `sheet_drag_handle`) peeking `sheet_peek_height` out of the bottom, dragged (`sheet_swipe_enabled`) or flung
/// up to its height, and `snackbar_host` given the state's host state, above the half open sheet (else at the
/// bottom). `scaffold_state` (None: a remembered [`remember_bottom_sheet_scaffold_state`]) holds the sheet's
/// state and the snackbar host's; the whole on `container_color` with `content_color` (None: the container's
/// content color) the content color.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn BottomSheetScaffold(
    sheet_content: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] scaffold_state: Option<BottomSheetScaffoldState>,
    #[default(BottomSheetDefaults::SheetPeekHeight)] sheet_peek_height: Dp,
    #[default(BottomSheetDefaults::SheetMaxWidth)] sheet_max_width: Dp,
    #[default(BottomSheetDefaults::ExpandedShape())] sheet_shape: Arc<dyn Shape>,
    #[default(BottomSheetDefaults::ContainerColor())] sheet_container_color: Color,
    #[default(None)] sheet_content_color: Option<Color>,
    #[default(Dp(0.0))] sheet_tonal_elevation: Dp,
    #[default(BottomSheetDefaults::Elevation)] sheet_shadow_elevation: Dp,
    #[default(default_drag_handle())] sheet_drag_handle: Option<Rc<dyn Fn()>>,
    #[default(true)] sheet_swipe_enabled: bool,
    #[default(None)] top_bar: Option<Rc<dyn Fn()>>,
    #[default(default_snackbar_host)] snackbar_host: impl Fn(SnackbarHostState) + 'static,
    #[default(MaterialTheme::color_scheme().surface)] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    content: impl Fn(PaddingValues) + 'static,
) {
    let scaffold_state = match scaffold_state {
        Some(scaffold_state) => scaffold_state,
        None => remember_bottom_sheet_scaffold_state(),
    };
    let sheet_content_color = sheet_content_color.unwrap_or_else(|| content_color_for(sheet_container_color));
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    Box(
        modifier = named_args!(modifier.fill_max_size().background(container_color)),
        content = || {
            // Using composition local provider instead of Surface as Surface implements .clip() which intercepts
            // touch events in testing.
            KompozitionLocalProvider(&[LocalContentColor.provides(content_color)], || {
                let sheet_state = scaffold_state.bottom_sheet_state.clone();
                let snackbar_host_state = scaffold_state.snackbar_host_state.clone();
                let offset_state = sheet_state.clone();
                let standard_state = sheet_state.clone();
                BottomSheetScaffoldLayout(
                    top_bar = top_bar.clone(),
                    body = Rc::new(move || content(named_args!(PaddingValues(bottom = sheet_peek_height))))
                        as Rc<dyn Fn()>,
                    snackbar_host = Rc::new(move || snackbar_host(snackbar_host_state.clone())) as Rc<dyn Fn()>,
                    sheet_offset = Rc::new(move || offset_state.require_offset()) as Rc<dyn Fn() -> f32>,
                    sheet_state = sheet_state,
                    bottom_sheet = Rc::new(move || {
                        StandardBottomSheet(
                            state = standard_state.clone(),
                            peek_height = sheet_peek_height,
                            sheet_max_width = sheet_max_width,
                            sheet_swipe_enabled = sheet_swipe_enabled,
                            shape = sheet_shape.clone(),
                            container_color = sheet_container_color,
                            content_color = sheet_content_color,
                            tonal_elevation = sheet_tonal_elevation,
                            shadow_elevation = sheet_shadow_elevation,
                            drag_handle = sheet_drag_handle.clone(),
                            content = || sheet_content(),
                        );
                    }) as Rc<dyn Fn()>,
                );
            });
        },
    );
}

/// The default snackbar host (Kotlin's `{ SnackbarHost(it) }`).
#[track_caller]
fn default_snackbar_host(host_state: SnackbarHostState) {
    kompozable_block!({
        SnackbarHost(host_state);
    })
}

/// The state of a [`BottomSheetScaffold`] (Compose's `BottomSheetScaffoldState`): its sheet's state and its
/// snackbar host's.
#[derive(Clone, PartialEq, KotlinProperties)]
pub struct BottomSheetScaffoldState {
    pub bottom_sheet_state: SheetState,
    pub snackbar_host_state: SnackbarHostState,
}

/// Compose's `BottomSheetScaffoldState(bottomSheetState, snackbarHostState)`.
#[named(type)]
pub fn BottomSheetScaffoldState(
    bottom_sheet_state: SheetState,
    snackbar_host_state: SnackbarHostState,
) -> BottomSheetScaffoldState {
    BottomSheetScaffoldState { bottom_sheet_state, snackbar_host_state }
}

/// A bottom sheet scaffold's state of `bottom_sheet_state` (None: a remembered [`remember_bottom_sheet_state`]
/// partially expanded to start with, never hidden: material3 1.5.0-alpha20's default) and `snackbar_host_state` (None: one remembered), remembered while
/// they are the same (Compose's `rememberBottomSheetScaffoldState`).
#[kompozable]
pub fn remember_bottom_sheet_scaffold_state(
    #[default(None)] bottom_sheet_state: Option<SheetState>,
    #[default(None)] snackbar_host_state: Option<SnackbarHostState>,
) -> BottomSheetScaffoldState {
    let bottom_sheet_state = match bottom_sheet_state {
        Some(state) => state,
        None => remember_bottom_sheet_state(
            initial_value = PartiallyExpanded,
            enabled_values = set_of([PartiallyExpanded, Expanded]),
        ),
    };
    let snackbar_host_state = match snackbar_host_state {
        Some(state) => state,
        None => remember(SnackbarHostState::new),
    };
    remember_keyed((bottom_sheet_state.clone(), snackbar_host_state.clone()), move || BottomSheetScaffoldState {
        bottom_sheet_state,
        snackbar_host_state,
    })
}

/// A standard bottom sheet's state, saved and restored (Compose's `rememberStandardBottomSheetState`, deprecated by
/// material3 1.5.0-alpha20): at `initial_value` to start with; `confirm_value_change` may veto a change (a new
/// closure, by what it captures, makes a new state); `skip_hidden_state`, it never hides.
/// [`remember_bottom_sheet_state`] of the values it doesn't skip.
#[deprecated(note = "Use rememberBottomSheetState with PartiallyExpanded initial value")]
#[kompozable]
pub fn remember_standard_bottom_sheet_state(
    #[default(PartiallyExpanded)] initial_value: SheetValue,
    #[default(|_| true)]
    #[memoized]
    confirm_value_change: impl Fn(SheetValue) -> bool + 'static,
    #[default(true)] skip_hidden_state: bool,
) -> SheetState {
    let enabled_values = if skip_hidden_state {
        set_of([PartiallyExpanded, Expanded])
    } else {
        set_of([Hidden, PartiallyExpanded, Expanded])
    };
    // (Compose: `rememberBottomSheetState(…)`, rememberSheetState of the same arguments: here of the closure's
    // identity as this function's memoized parameter carries it)
    let id = confirm_value_change.id();
    let confirm: Rc<dyn Fn(SheetValue) -> bool> = Rc::new(confirm_value_change.into_value());
    remember_sheet_state(
        enabled_values = enabled_values,
        confirm_value_change = Memoized::new(confirm, id),
        initial_value = initial_value,
    )
}

/// The scaffold's sheet (Compose's private `StandardBottomSheet`).
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn StandardBottomSheet(
    state: SheetState,
    peek_height: Dp,
    sheet_max_width: Dp,
    sheet_swipe_enabled: bool,
    shape: Arc<dyn Shape>,
    container_color: Color,
    content_color: Color,
    tonal_elevation: Dp,
    shadow_elevation: Dp,
    drag_handle: Option<Rc<dyn Fn()>>,
    content: impl Fn() + 'static,
) {
    // TODO Load the motionScheme tokens from the component tokens file
    let motion_scheme = MaterialTheme::motion_scheme();
    let show_motion = motion_scheme.default_spatial_spec::<f32>();
    let hide_motion = motion_scheme.fast_effects_spec::<f32>();
    let spatial_fling_spec = remembered_default_spatial_spec();
    {
        let (state, spec) = (state.clone(), motion_scheme.default_spatial_spec::<f32>());
        SideEffect(move || {
            state.set_show_motion_spec(show_motion);
            state.set_hide_motion_spec(hide_motion);
            state.set_anchored_draggable_motion_spec(spec);
        });
    }

    let scope = remember_coroutine_scope();
    let orientation = Orientation::Vertical;
    let peek_height_px = LocalDensity.current().to_px(peek_height);
    let anchored_draggable_fling_behavior = AnchoredDraggableDefaults::fling_behavior(
        state.anchored_draggable_state(),
        Some(remembered_positional_threshold(&state)),
        Some(spatial_fling_spec),
    );

    let nested_scroll = if sheet_swipe_enabled {
        let (s, fling) = (state.clone(), anchored_draggable_fling_behavior.clone() as Rc<dyn FlingBehavior>);
        let connection = remember_keyed(state.anchored_draggable_state(), move || {
            Rc::new(ConsumeSwipeWithinBottomSheetBoundsNestedScrollConnection(s, orientation, fling))
                as Rc<dyn NestedScrollConnection>
        });
        named_args!(Modifier.nested_scroll(connection))
    } else {
        Modifier
    };
    let anchors_state = state.clone();
    let surface_modifier = named_args!(Modifier.width_in(max = sheet_max_width))
        .fill_max_width()
        .then(named_args!(Modifier.required_height_in(min = peek_height)))
        .then(nested_scroll)
        .draggable_anchors(state.anchored_draggable_state(), orientation, move |sheet_size, constraints| {
            let state = &anchors_state;
            let layout_height = constraints.max_height as f32;
            let sheet_height = sheet_size.height as f32;

            let new_anchors = DraggableAnchors(|a: &mut DraggableAnchorsConfig<SheetValue>| {
                let is_hidden_anchor_available =
                    sheet_height == 0.0 || peek_height_px == 0.0 || !state.skip_hidden_state();

                // We are preserving ambiguous anchor reconciliation for first layout pass. This handles the use
                // case where sheetPeekHeight is backed by a mutable value which is backed by 0.dp before being
                // recalculated. We can assume the state is in its first pass by asserting anchor sizes are zero, as
                // we enforce at least 1 anchor below. We then settle at partial as this is the anchor external users
                // have access to via sheetPeekHeight API.
                let is_initial_layout = state.anchored_draggable_state().anchors().size() == 0;
                let is_stable_at_partial = state.current_value() == PartiallyExpanded && !state.is_animation_running();

                let is_ambiguous_partial_allowed = peek_height_px == 0.0 && (is_initial_layout || is_stable_at_partial);

                let is_partially_expanded_anchor_available = !state.skip_partially_expanded()
                    && (peek_height_px > 0.0 || is_ambiguous_partial_allowed)
                    && peek_height_px != sheet_height;

                let is_expanded_anchor_available = sheet_height > 0.0;

                if !(is_hidden_anchor_available
                    || is_partially_expanded_anchor_available
                    || is_expanded_anchor_available)
                {
                    panic_kt(
                        "IllegalArgumentException",
                        "BottomSheetScaffold: Require at least 1 anchor to be initialized",
                    );
                }

                if is_partially_expanded_anchor_available {
                    a.at(PartiallyExpanded, layout_height - peek_height_px);
                }
                if is_hidden_anchor_available {
                    a.at(Hidden, layout_height);
                }
                if is_expanded_anchor_available {
                    a.at(Expanded, layout_height - sheet_height);
                }
            });
            let new_target = match state.target_value() {
                Hidden => Hidden,
                old_target @ PartiallyExpanded => {
                    if new_anchors.has_position_for(&PartiallyExpanded) {
                        PartiallyExpanded
                    } else if new_anchors.has_position_for(&Expanded) {
                        Expanded
                    } else if new_anchors.has_position_for(&Hidden) {
                        Hidden
                    } else {
                        old_target
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
            // (Kotlin's `Hidden -> if (newAnchors.hasPositionFor(Hidden)) Hidden else oldTarget`: the old target
            // was Hidden either way)
            (Arc::new(new_anchors) as Arc<dyn DraggableAnchors<SheetValue>>, new_target)
        });
    let surface_modifier = named_args!(surface_modifier.anchored_draggable(
        state.anchored_draggable_state(),
        orientation,
        enabled = sheet_swipe_enabled,
        fling_behavior = Some(anchored_draggable_fling_behavior as Rc<dyn FlingBehavior>),
    ))
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
                    // Scale the content down in case the sheet offset overflows below the min anchor. The wrapping
                    // Surface is scaled up, so this is done to maintain the content's aspect ratio.
                    .vertical_scale_down(state.clone()),
                content = || {
                    if let Some(drag_handle) = drag_handle.clone() {
                        let partial_expand_action_label = get_string(Strings::BottomSheetPartialExpandDescription);
                        let dismiss_action_label = get_string(Strings::BottomSheetDismissDescription);
                        let expand_action_label = get_string(Strings::BottomSheetExpandDescription);
                        let (click_state, click_scope) = (state.clone(), scope.clone());
                        let (sem_state, sem_scope) = (state.clone(), scope.clone());
                        DragHandleWithTooltip(
                            modifier = Modifier
                                .clickable(move || {
                                    let s = click_state.clone();
                                    match click_state.current_value() {
                                        Expanded => named_args!(click_scope.launch(async move {
                                            if !s.skip_hidden_state() {
                                                s.hide().await;
                                            } else {
                                                s.partial_expand().await;
                                            }
                                        })),
                                        PartiallyExpanded => {
                                            named_args!(click_scope.launch(async move { s.expand().await }))
                                        }
                                        _ => named_args!(click_scope.launch(async move { s.show().await })),
                                    };
                                })
                                .semantics_with(true, move |c| {
                                    let state = &sem_state;
                                    // Provides semantics to interact with the bottomsheet if there is more than one
                                    // anchor to swipe to and swiping is enabled.
                                    if state.anchored_draggable_state().anchors().size() > 1 && sheet_swipe_enabled {
                                        if state.current_value() == PartiallyExpanded {
                                            let (s, scope) = (state.clone(), sem_scope.clone());
                                            named_args!(c.expand(
                                                label = Some(expand_action_label.clone()),
                                                move || {
                                                    let can_expand = s.confirm_value_change(Expanded);
                                                    if can_expand {
                                                        let s = s.clone();
                                                        named_args!(scope.launch(async move { s.expand().await }));
                                                    }
                                                    can_expand
                                                }
                                            ));
                                        } else {
                                            let (s, scope) = (state.clone(), sem_scope.clone());
                                            named_args!(c.collapse(
                                                label = Some(partial_expand_action_label.clone()),
                                                move || {
                                                    let can_partially_expand =
                                                        s.confirm_value_change(PartiallyExpanded);
                                                    let s = s.clone();
                                                    named_args!(scope.launch(async move { s.partial_expand().await }));
                                                    can_partially_expand
                                                }
                                            ));
                                        }
                                        if !state.skip_hidden_state() {
                                            let (s, scope) = (state.clone(), sem_scope.clone());
                                            named_args!(c.dismiss(
                                                label = Some(dismiss_action_label.clone()),
                                                move || {
                                                    let can_hide = s.confirm_value_change(Hidden);
                                                    let s = s.clone();
                                                    named_args!(scope.launch(async move { s.hide().await }));
                                                    can_hide
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

/// The scaffold's layout (Compose's private `BottomSheetScaffoldLayout`): the body below the top bar, the sheet
/// centered on top (placed at the top: its anchors move it), the snackbar host centered above the half open sheet,
/// else at the bottom. The sheet's offset and value are read as it places, so a moving sheet places it again
/// without composing anything.
#[kompozable]
fn BottomSheetScaffoldLayout(
    top_bar: Option<Rc<dyn Fn()>>,
    body: Rc<dyn Fn()>,
    bottom_sheet: Rc<dyn Fn()>,
    snackbar_host: Rc<dyn Fn()>,
    sheet_offset: Rc<dyn Fn() -> f32>,
    sheet_state: SheetState,
) {
    // (Compose's measure policy lambda, memoized by what it captures: the sheet's state and its offset reader)
    let measure_policy = remember_keyed(sheet_state.clone(), move || {
        Rc::new(BottomSheetScaffoldMeasurePolicy { sheet_offset, sheet_state }) as Rc<dyn MultiContentMeasurePolicy>
    });
    MultiContentLayout(
        contents = vec![top_bar.unwrap_or_else(|| Rc::new(|| {})), body, bottom_sheet, snackbar_host],
        measure_policy = measure_policy,
    );
}

/// [`BottomSheetScaffoldLayout`]'s measure policy.
struct BottomSheetScaffoldMeasurePolicy {
    sheet_offset: Rc<dyn Fn() -> f32>,
    sheet_state: SheetState,
}

impl MultiContentMeasurePolicy for BottomSheetScaffoldMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[Vec<&dyn Measurable>],
        constraints: Constraints,
    ) -> MeasureResult {
        let [top_bar_measurables, body_measurables, bottom_sheet_measurables, snackbar_host_measurables] = measurables
        else {
            unreachable!("four contents")
        };
        let layout_width = constraints.max_width;
        let layout_height = constraints.max_height;
        let loose_constraints = Constraints { min_width: 0, min_height: 0, ..constraints };

        let sheet_placeables: Vec<_> = bottom_sheet_measurables.iter().map(|m| m.measure(loose_constraints)).collect();

        let top_bar_placeables: Vec<_> = top_bar_measurables.iter().map(|m| m.measure(loose_constraints)).collect();
        let top_bar_height = top_bar_placeables.iter().map(|p| p.height()).max().unwrap_or(0);

        let body_constraints = Constraints { max_height: layout_height - top_bar_height, ..loose_constraints };
        let body_placeables: Vec<_> = body_measurables.iter().map(|m| m.measure(body_constraints)).collect();

        let snackbar_placeables: Vec<_> =
            snackbar_host_measurables.iter().map(|m| m.measure(loose_constraints)).collect();

        let (sheet_offset, sheet_state) = (self.sheet_offset.clone(), self.sheet_state.clone());
        scope.layout(layout_width, layout_height, move |p| {
            let sheet_width = sheet_placeables.iter().map(|p| p.width()).max().unwrap_or(0);
            let sheet_offset_x = 0.max((layout_width - sheet_width) / 2);

            let snackbar_width = snackbar_placeables.iter().map(|p| p.width()).max().unwrap_or(0);
            let snackbar_height = snackbar_placeables.iter().map(|p| p.height()).max().unwrap_or(0);
            let snackbar_offset_x = (layout_width - snackbar_width) / 2;
            let snackbar_offset_y = match sheet_state.current_value() {
                PartiallyExpanded => round_to_int(sheet_offset()) - snackbar_height,
                Expanded | Hidden => layout_height - snackbar_height,
            };

            // Placement order is important for elevation
            for placeable in &body_placeables {
                named_args!(p.place_relative(placeable, 0, top_bar_height));
            }
            for placeable in &top_bar_placeables {
                named_args!(p.place_relative(placeable, 0, 0));
            }
            for placeable in &sheet_placeables {
                named_args!(p.place_relative(placeable, sheet_offset_x, 0));
            }
            for placeable in &snackbar_placeables {
                named_args!(p.place_relative(placeable, snackbar_offset_x, snackbar_offset_y));
            }
        })
    }
}

/// Kotlin's `Float.roundToInt` (Math.round: a half up).
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}
