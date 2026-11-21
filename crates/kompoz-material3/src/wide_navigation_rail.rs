//! Material's wide navigation rail, as Compose's WideNavigationRail.kt (material3 1.5.0-alpha17) with Compose for
//! desktop's actuals (WideNavigationRail.skiko.kt): a rail of destinations down the start of a wide screen that
//! collapses (96 dp, each icon over its label) or expands (220 to 360 dp, each label after its icon) by its
//! [`WideNavigationRailState`] ([`WideNavigationRail`]); its modal form ([`ModalWideNavigationRail`]) that opens
//! over the content in a dialog with a scrim, or slides in from off screen; its items ([`WideNavigationRailItem`],
//! moving between the two layouts as the rail does), colors ([`WideNavigationRailColors`]), defaults and the
//! override API of both rails.

use crate::color_scheme::{ColorScheme, DisabledAlpha, content_color_for};
use crate::interactive_component_size::LocalMinimumInteractiveComponentSize;
use crate::internal::draggable_anchors::DraggableAnchorsModifiers;
use crate::internal::strings::{Strings, get_string};
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::material_theme::MaterialTheme;
use crate::navigation_item::{
    AnimatedNavigationItem, DynamicPaddingValues, NavigationItemColors, NavigationItemIconPosition,
};
use crate::scrim::Scrim;
use crate::surface::Surface;
use crate::tokens::{
    MotionSchemeKeyTokens, NavigationRailBaselineItemTokens, NavigationRailCollapsedTokens, NavigationRailColorTokens,
    NavigationRailExpandedTokens, NavigationRailHorizontalItemTokens, NavigationRailVerticalItemTokens, ScrimTokens,
};
use crate::wide_navigation_rail_state::{
    ModalWideNavigationRailState, RailPredictiveBackState, WideNavigationRailState, WideNavigationRailValue,
    remember_wide_navigation_rail_state,
};
use kompoz_animation::{Animatable, AnimationSpec, AnimationVector1D, animate_dp_as_state, animate_float_as_state};
use kompoz_coroutines::{Channel, CoroutineScope, LaunchedEffect, coroutine_scope, remember_coroutine_scope};
use kompoz_foundation::gestures::{
    AnchoredDraggableDefaults, DraggableAnchors, DraggableAnchorsConfig, FlingBehavior, LocalFuture, Orientation,
    ScrollScope, TargetedFlingBehavior,
};
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::layout::{PaddingValues, VerticalArrangement, WindowInsets, WindowInsetsSides};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Constraints, Dp, LayoutDirection, Shape, lerp};
use kompoz_runtime::{
    KompozitionLocal, KotlinProperties, MovableContent, SideEffect, State, derived_state_of, kompozable,
    kompozable_block, movable_content_of, mutable_int_state_of, named, named_args, remember, remember_keyed,
};
use kompoz_ui::Modifier;
use kompoz_ui::input::{Key, KeyEventType};
use kompoz_ui::layout::{GraphicsLayerScope, Layout, Placeable, TransformOrigin, measure_policy};
use kompoz_ui::platform::LocalLayoutDirection;
use kompoz_ui::window::{Dialog, DialogProperties};
use std::rc::Rc;
use std::sync::Arc;

// ---- WideNavigationRail -----------------------------------------------------------------------------------------

/// A wide navigation rail (Compose's `WideNavigationRail`): `header` (if any) at the top, then `content`
/// (typically [`WideNavigationRailItem`]s) arranged by `arrangement` in the room below it (in the whole height for
/// [`Arrangement::Center`]), in a [`Surface`] of `shape` and `colors`' container and content colors, as tall as it
/// may be, padded by `window_insets` and `content_padding`, a selectable group and a traversal group to
/// accessibility. Collapsed (`state`'s target value) it is 96 dp wide, its items 4 dp apart and at least 64 dp
/// tall; expanded at least 220 dp (as wide as its widest item and 20 dp, or its header, up to 360 dp), its items
/// together and at least the minimum interactive size — the width and the spacing animated on the default spatial
/// motion. `state` None is Kotlin's default, a [`remember_wide_navigation_rail_state`]. Made by
/// [`LocalWideNavigationRailOverride`]'s override ([`DefaultWideNavigationRailOverride`] unless provided).
///
/// Kotlin: `WideNavigationRail(modifier: Modifier = Modifier, state: WideNavigationRailState = rememberWideNavigationRailState(), shape: Shape = WideNavigationRailDefaults.shape, colors: WideNavigationRailColors = WideNavigationRailDefaults.colors(), header: @Composable (() -> Unit)? = null, windowInsets: WindowInsets = WideNavigationRailDefaults.windowInsets, arrangement: Arrangement.Vertical = WideNavigationRailDefaults.arrangement, contentPadding: PaddingValues = WideNavigationRailDefaults.ContentPadding, content: @Composable () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn WideNavigationRail(
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<Rc<dyn WideNavigationRailState>>,
    #[default(WideNavigationRailDefaults::shape())] shape: Arc<dyn Shape>,
    #[default(WideNavigationRailDefaults.colors())] colors: WideNavigationRailColors,
    #[default(None)] header: Option<Rc<dyn Fn()>>,
    #[default(WideNavigationRailDefaults::window_insets())] window_insets: WindowInsets,
    #[default(WideNavigationRailDefaults::arrangement())] arrangement: VerticalArrangement,
    #[default(WideNavigationRailDefaults::ContentPadding())] content_padding: PaddingValues,
    content: impl Fn() + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_wide_navigation_rail_state(),
    };
    let override_ = LocalWideNavigationRailOverride.current();
    override_.WideNavigationRail(&WideNavigationRailOverrideScope {
        modifier,
        state,
        shape,
        colors,
        header,
        window_insets,
        arrangement,
        content_padding,
        content: Rc::new(content),
    });
}

/// The wide navigation rail of an override (Compose's experimental `WideNavigationRailOverride`): what makes every
/// [`WideNavigationRail`] below a [`LocalWideNavigationRailOverride`] providing it, from the scope's arguments.
pub trait WideNavigationRailOverride {
    /// Composes the rail of `scope`'s arguments (Compose's `WideNavigationRailOverrideScope.WideNavigationRail()`).
    fn WideNavigationRail(&self, scope: &WideNavigationRailOverrideScope);
}

impl PartialEq for dyn WideNavigationRailOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default way a wide navigation rail is made (Compose's experimental `DefaultWideNavigationRailOverride`):
/// collapsed or expanded as its state's target value says.
#[derive(Clone, Copy, Debug)]
pub struct DefaultWideNavigationRailOverride;

impl WideNavigationRailOverride for DefaultWideNavigationRailOverride {
    #[track_caller]
    fn WideNavigationRail(&self, scope: &WideNavigationRailOverrideScope) {
        let scope = scope.clone();
        kompozable_block!({
            WideNavigationRailLayout(
                scope.modifier.clone(),
                false,
                scope.state.target_value().is_expanded(),
                scope.colors,
                scope.shape.clone(),
                scope.header.clone(),
                scope.window_insets.clone(),
                scope.arrangement.clone(),
                scope.content_padding.clone(),
                scope.content.clone(),
            );
        })
    }
}

/// The arguments of a [`WideNavigationRail`], given to its override (Compose's experimental
/// `WideNavigationRailOverrideScope`). Made by [`WideNavigationRail`].
#[derive(Clone)]
pub struct WideNavigationRailOverrideScope {
    modifier: Modifier,
    state: Rc<dyn WideNavigationRailState>,
    shape: Arc<dyn Shape>,
    colors: WideNavigationRailColors,
    header: Option<Rc<dyn Fn()>>,
    window_insets: WindowInsets,
    arrangement: VerticalArrangement,
    content_padding: PaddingValues,
    content: Rc<dyn Fn()>,
}

impl WideNavigationRailOverrideScope {
    /// The rail's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The rail's state (Compose's `state`).
    pub fn state(&self) -> Rc<dyn WideNavigationRailState> {
        self.state.clone()
    }

    /// The rail's shape (Compose's `shape`).
    pub fn shape(&self) -> Arc<dyn Shape> {
        self.shape.clone()
    }

    /// The rail's colors (Compose's `colors`).
    pub fn colors(&self) -> WideNavigationRailColors {
        self.colors
    }

    /// The rail's header, if any (Compose's `header`).
    pub fn header(&self) -> Option<Rc<dyn Fn()>> {
        self.header.clone()
    }

    /// The insets the rail keeps clear of (Compose's `windowInsets`).
    pub fn window_insets(&self) -> WindowInsets {
        self.window_insets.clone()
    }

    /// How the rail arranges its items (Compose's `arrangement`).
    pub fn arrangement(&self) -> VerticalArrangement {
        self.arrangement.clone()
    }

    /// The padding inside the rail (Compose's `contentPadding`).
    pub fn content_padding(&self) -> PaddingValues {
        self.content_padding.clone()
    }

    /// The rail's items (Compose's `content`).
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the wide navigation rails below (Compose's experimental
/// `LocalWideNavigationRailOverride`): [`DefaultWideNavigationRailOverride`] unless provided.
pub static LocalWideNavigationRailOverride: KompozitionLocal<Rc<dyn WideNavigationRailOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultWideNavigationRailOverride));

/// The rail itself (Compose's private `WideNavigationRailLayout`): a [`Surface`] (the modal colors if `is_modal`)
/// holding the header and the items, collapsed or `expanded`, its widths animated (on the fast spatial motion for
/// the modal rail), as [`WideNavigationRail`] says.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn WideNavigationRailLayout(
    modifier: Modifier,
    is_modal: bool,
    expanded: bool,
    colors: WideNavigationRailColors,
    shape: Arc<dyn Shape>,
    header: Option<Rc<dyn Fn()>>,
    window_insets: WindowInsets,
    arrangement: VerticalArrangement,
    content_padding: PaddingValues,
    content: Rc<dyn Fn()>,
) {
    let current_width = remember(|| mutable_int_state_of(0));
    let actual_max_expanded_width = remember(|| mutable_int_state_of(0));
    let minimum_a11y_size = if LocalMinimumInteractiveComponentSize.current() == Dp::Unspecified {
        Dp(0.0)
    } else {
        LocalMinimumInteractiveComponentSize.current()
    };

    // TODO: Load the motionScheme tokens from the component tokens file.
    let animation_spec: Arc<dyn AnimationSpec<Dp>> = MotionSchemeKeyTokens::DefaultSpatial.value::<Dp>();
    let modal_animation_spec: Arc<dyn AnimationSpec<Dp>> = MotionSchemeKeyTokens::FastSpatial.value::<Dp>();
    let min_width: State<Dp> = animate_dp_as_state(
        if !expanded { CollapsedRailWidth } else { ExpandedRailMinWidth },
        animation_spec = if !is_modal { animation_spec.clone() } else { modal_animation_spec.clone() },
    );
    let width_full_range: State<Dp> = animate_dp_as_state(
        if !expanded { CollapsedRailWidth } else { ExpandedRailMaxWidth },
        animation_spec = if !is_modal { animation_spec.clone() } else { modal_animation_spec.clone() },
    );
    let item_vertical_spaced_by: State<Dp> = animate_dp_as_state(
        if !expanded { NavigationRailCollapsedTokens::ItemVerticalSpace } else { Dp(0.0) },
        animation_spec = animation_spec.clone(),
    );
    let item_min_height: State<Dp> = animate_dp_as_state(
        if !expanded { TopIconItemMinHeight } else { minimum_a11y_size },
        animation_spec = animation_spec.clone(),
    );

    let has_header = header.is_some();
    Surface(
        color = if !is_modal { colors.container_color } else { colors.modal_container_color },
        content_color = Some(if !is_modal { colors.content_color } else { colors.modal_content_color }),
        shape = shape,
        modifier = modifier,
        content = || {
            let (header, content, arrangement) = (header.clone(), content.clone(), arrangement.clone());
            Layout(
                modifier = named_args!(
                    Modifier
                        .fill_max_height()
                        .window_insets_padding(window_insets.clone())
                        .width_in(max = ExpandedRailMaxWidth)
                )
                .padding_values(content_padding.clone())
                .selectable_group()
                .semantics_with(false, |c| c.set_is_traversal_group(true)),
                content = move || {
                    if let Some(header) = header.clone() {
                        Box(modifier = Modifier.layout_id(HeaderLayoutIdTag), content = move || header());
                    }
                    content();
                },
                measure_policy = measure_policy(move |scope, measurables, constraints| {
                    let height = constraints.max_height;
                    let mut items_count = measurables.len();
                    let mut actual_expanded_min_width = constraints.min_width;
                    let actual_min_width = if constraints.min_width == 0 {
                        actual_expanded_min_width = scope.round_to_px(ExpandedRailMinWidth).min(constraints.max_width);
                        scope.round_to_px(min_width.value()).min(constraints.max_width)
                    } else {
                        constraints.min_width
                    };
                    // If there are no items, rail will be empty.
                    if items_count < 1 {
                        return scope.layout(actual_min_width, height, |_| {});
                    }
                    let loose_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));
                    let mut items_measurables = measurables;

                    let mut constraints_offset = 0;
                    let mut header_placeable: Option<Placeable> = None;
                    if has_header {
                        let placeable = measurables
                            .iter()
                            .find(|m| m.layout_id().is_some_and(|id| id.is(&HeaderLayoutIdTag)))
                            .expect("WideNavigationRailLayout: its header")
                            .measure(loose_constraints);
                        // Header is always first element in measurables list.
                        if items_count > 1 {
                            items_measurables = &measurables[1..items_count];
                        }
                        // Real item count doesn't include the header.
                        items_count -= 1;
                        constraints_offset = placeable.height();
                        header_placeable = Some(placeable);
                    }

                    let mut items_placeables: Option<Vec<Placeable>> =
                        if items_count > 0 { Some(Vec::new()) } else { None };
                    let item_expanded_max_width =
                        loose_constraints.max_width - scope.round_to_px(WNRItemHorizontalPadding);
                    let item_max_width_constraint = (if expanded { item_expanded_max_width } else { actual_min_width })
                        .max(scope.round_to_px(minimum_a11y_size));
                    let item_max_height_constraint =
                        loose_constraints.max_height.max(scope.round_to_px(item_min_height.value()));
                    let mut expanded_item_max_width = 0;
                    if let Some(items_placeables) = items_placeables.as_mut() {
                        for it in items_measurables {
                            let measured_item =
                                it.measure(Constraints::offset(&loose_constraints, 0, -constraints_offset).constrain(
                                    Constraints::fit_prioritizing_width(
                                        scope.round_to_px(minimum_a11y_size),
                                        item_max_width_constraint,
                                        scope.round_to_px(item_min_height.value()),
                                        item_max_height_constraint,
                                    ),
                                ));
                            let max_item_width = measured_item.measured_width();
                            if expanded && expanded_item_max_width < max_item_width {
                                expanded_item_max_width = max_item_width + scope.round_to_px(WNRItemHorizontalPadding);
                            }
                            // (Compose's: the offset of the last item measured, not of all of them)
                            constraints_offset = measured_item.height();
                            items_placeables.push(measured_item);
                        }
                    }

                    let mut width = actual_min_width;
                    // Limit collapsed rail to fixed width, but expanded rail can be as wide as
                    // constraints.maxWidth
                    if expanded {
                        let widest_element_width =
                            expanded_item_max_width.max(header_placeable.as_ref().map_or(0, |p| p.width()));

                        if widest_element_width > actual_min_width && widest_element_width > actual_expanded_min_width {
                            let width_constrain =
                                widest_element_width.max(actual_expanded_min_width).min(constraints.max_width);
                            // Use widthFullRange so there's no jump in animation for when the
                            // expanded width has to be wider than actualExpandedMinWidth.
                            width = scope.round_to_px(width_full_range.value()).min(width_constrain);
                            actual_max_expanded_width.set(width);
                        }
                    } else if actual_max_expanded_width.peek() > 0 {
                        // Use widthFullRange so there's no jump in animation for the case
                        // when the expanded width was wider than actualExpandedMinWidth.
                        width = scope
                            .round_to_px(width_full_range.value())
                            .clamp(actual_min_width, current_width.peek().max(actual_min_width));
                    }
                    current_width.set(width);

                    let header_padding = scope.round_to_px(WNRHeaderPadding);
                    let spaced_by = scope.round_to_px(item_vertical_spaced_by.value());
                    let density = **scope;
                    let arrangement = arrangement.clone();
                    scope.layout(width, height, move |p| {
                        let mut header_offset = 0;
                        if let Some(header_placeable) = header_placeable.as_ref().filter(|h| h.height() > 0) {
                            p.place_relative(header_placeable, 0, 0);
                            header_offset += header_placeable.height() + header_padding;
                        }

                        if let Some(items_placeables) = &items_placeables {
                            let is_center = arrangement == Arrangement::Center;
                            let layout_size = if is_center {
                                // For centered arrangement the items will be centered in
                                // the container, not in the remaining space below the
                                // header.
                                height
                            } else {
                                height - header_offset
                            };
                            let last = items_placeables.len() - 1;
                            let sizes: Vec<i32> = items_placeables
                                .iter()
                                .enumerate()
                                .map(|(index, item)| item.height() + if index < last { spaced_by } else { 0 })
                                .collect();
                            let mut y = vec![0; items_placeables.len()];
                            arrangement.arrange(&density, layout_size, &sizes, &mut y);

                            let offset = if is_center { 0 } else { header_offset };
                            for (item, y) in items_placeables.iter().zip(&y) {
                                p.place_relative(item, 0, y + offset);
                            }
                        }
                    })
                }),
            );
        },
    );
}

// ---- ModalWideNavigationRail ------------------------------------------------------------------------------------

/// A modal wide navigation rail (Compose's `ModalWideNavigationRail`): collapsed, a collapsed
/// [`WideNavigationRail`] in place (of `collapsed_shape`); expanding, a second rail (of `expanded_shape`, the modal
/// colors, `expanded_header_top_padding` above its header) in a dialog over the whole window, over a scrim of
/// `colors`' modal scrim color that collapses it when tapped, and it collapses by back or Escape too (as
/// `expanded_properties` say); the one in place stays, empty, so that the screen's layout doesn't change. With
/// `hide_on_collapse` nothing stays in place: the rail slides in from off screen and out again, and can be swiped
/// away. `state` None is Kotlin's default, a [`remember_wide_navigation_rail_state`]. Made by
/// [`LocalModalWideNavigationRailOverride`]'s override ([`DefaultModalWideNavigationRailOverride`] unless
/// provided).
///
/// Kotlin: `ModalWideNavigationRail(modifier: Modifier = Modifier, state: WideNavigationRailState = rememberWideNavigationRailState(), hideOnCollapse: Boolean = false, collapsedShape: Shape = WideNavigationRailDefaults.modalCollapsedShape, expandedShape: Shape = WideNavigationRailDefaults.modalExpandedShape, colors: WideNavigationRailColors = WideNavigationRailDefaults.colors(), header: @Composable (() -> Unit)? = null, expandedHeaderTopPadding: Dp = 0.dp, windowInsets: WindowInsets = WideNavigationRailDefaults.windowInsets, arrangement: Arrangement.Vertical = WideNavigationRailDefaults.arrangement, expandedProperties: ModalWideNavigationRailProperties = WideNavigationRailDefaults.ModalExpandedProperties, contentPadding: PaddingValues = WideNavigationRailDefaults.ContentPadding, content: @Composable () -> Unit)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ModalWideNavigationRail(
    #[default(Modifier)] modifier: Modifier,
    #[default(None)] state: Option<Rc<dyn WideNavigationRailState>>,
    #[default(false)] hide_on_collapse: bool,
    #[default(WideNavigationRailDefaults::modal_collapsed_shape())] collapsed_shape: Arc<dyn Shape>,
    #[default(WideNavigationRailDefaults::modal_expanded_shape())] expanded_shape: Arc<dyn Shape>,
    #[default(WideNavigationRailDefaults.colors())] colors: WideNavigationRailColors,
    #[default(None)] header: Option<Rc<dyn Fn()>>,
    #[default(Dp(0.0))] expanded_header_top_padding: Dp,
    #[default(WideNavigationRailDefaults::window_insets())] window_insets: WindowInsets,
    #[default(WideNavigationRailDefaults::arrangement())] arrangement: VerticalArrangement,
    #[default(WideNavigationRailDefaults::ModalExpandedProperties)]
    expanded_properties: ModalWideNavigationRailProperties,
    #[default(WideNavigationRailDefaults::ContentPadding())] content_padding: PaddingValues,
    content: impl Fn() + 'static,
) {
    let state = match state {
        Some(state) => state,
        None => remember_wide_navigation_rail_state(),
    };
    let scope = ModalWideNavigationRailOverrideScope {
        modifier,
        state,
        should_hide_on_collapse: hide_on_collapse,
        collapsed_shape,
        expanded_shape,
        colors,
        header,
        expanded_header_top_padding,
        window_insets,
        arrangement,
        expanded_properties,
        content_padding,
        content: Rc::new(content),
    };
    let override_ = LocalModalWideNavigationRailOverride.current();
    override_.ModalWideNavigationRail(&scope);
}

/// The modal wide navigation rail of an override (Compose's experimental `ModalWideNavigationRailOverride`): what
/// makes every [`ModalWideNavigationRail`] below a [`LocalModalWideNavigationRailOverride`] providing it, from the
/// scope's arguments.
pub trait ModalWideNavigationRailOverride {
    /// Composes the rail of `scope`'s arguments (Compose's
    /// `ModalWideNavigationRailOverrideScope.ModalWideNavigationRail()`).
    fn ModalWideNavigationRail(&self, scope: &ModalWideNavigationRailOverrideScope);
}

impl PartialEq for dyn ModalWideNavigationRailOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default way a modal wide navigation rail is made (Compose's experimental
/// `DefaultModalWideNavigationRailOverride`): as [`ModalWideNavigationRail`] says.
#[derive(Clone, Copy, Debug)]
pub struct DefaultModalWideNavigationRailOverride;

impl ModalWideNavigationRailOverride for DefaultModalWideNavigationRailOverride {
    #[track_caller]
    fn ModalWideNavigationRail(&self, scope: &ModalWideNavigationRailOverrideScope) {
        let ModalWideNavigationRailOverrideScope {
            modifier,
            state,
            should_hide_on_collapse,
            collapsed_shape,
            expanded_shape,
            colors,
            header,
            expanded_header_top_padding,
            window_insets,
            arrangement,
            expanded_properties,
            content_padding,
            content,
        } = scope.clone();
        kompozable_block!({
            let remember_content: Rc<dyn Fn()> = if should_hide_on_collapse {
                content.clone()
            } else {
                let movable: Rc<MovableContent> = remember_keyed(Rc::as_ptr(&content) as *const () as usize, || {
                    let content = content.clone();
                    Rc::new(movable_content_of(move || content()))
                });
                Rc::new(move || movable.call())
            };

            // TODO: Load the motionScheme tokens from the component tokens file.
            let modal_state_animation_spec: Arc<dyn AnimationSpec<f32>> =
                MotionSchemeKeyTokens::DefaultSpatial.value::<f32>();
            let modal_state: Rc<ModalWideNavigationRailState> = remember_keyed(state.clone(), || {
                Rc::new(ModalWideNavigationRailState::new(&*state, modal_state_animation_spec))
            });
            let position_progress: State<f32> = animate_float_as_state(
                if !state.target_value().is_expanded() { 0.0 } else { 1.0 },
                // TODO: Load the motionScheme tokens from the component tokens file.
                animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>(),
            );
            let is_collapsed = {
                let progress = position_progress.clone();
                remember(move || derived_state_of(move || progress.value() == 0.0))
            };
            let modal_expanded = {
                let progress = position_progress.clone();
                remember(move || derived_state_of(move || progress.value() >= 0.3))
            };
            let scope = remember_coroutine_scope();
            let animate_to_dismiss: Rc<dyn Fn()> = {
                let (scope, modal_state, state) = (scope.clone(), modal_state.clone(), state.clone());
                Rc::new(move || {
                    let (modal_state, state) = (modal_state.clone(), state.clone());
                    named_args!(scope.launch(async move {
                        if should_hide_on_collapse {
                            modal_state.collapse().await;
                        }
                        state.collapse().await;
                    }));
                })
            };
            let modal_animate_to_dismiss: Rc<dyn Fn() -> LocalFuture<'static, ()>> = {
                let (modal_state, state) = (modal_state.clone(), state.clone());
                Rc::new(move || {
                    let (modal_state, state) = (modal_state.clone(), state.clone());
                    Box::pin(async move {
                        if should_hide_on_collapse && !modal_state.target_value().is_expanded() {
                            state.collapse().await;
                        }
                    })
                })
            };

            // Display a non modal rail if it shouldn't hide on collapsed.
            if !should_hide_on_collapse {
                // Keep this rail even when expanded so that screen layout doesn't change.
                let (is_collapsed, remember_content) = (is_collapsed.clone(), remember_content.clone());
                WideNavigationRailLayout(
                    modifier.clone(),
                    false,
                    false,
                    colors,
                    collapsed_shape.clone(),
                    header.clone(),
                    window_insets.clone(),
                    arrangement.clone(),
                    content_padding.clone(),
                    Rc::new(move || {
                        // Only display content if it's collapsed, so that it doesn't affect this
                        // collapsed rail or the screen layout.
                        if is_collapsed.value() {
                            remember_content();
                        }
                    }),
                );
            }

            let channel: Channel<bool> = remember(Channel::conflated);
            if should_hide_on_collapse {
                let (channel, modal_state) = (channel.clone(), modal_state.clone());
                LaunchedEffect(channel.clone(), async move {
                    coroutine_scope(move |scope: CoroutineScope| async move {
                        while let Some(target) = channel.receive().await {
                            let new_target = channel.try_receive().unwrap_or(target);
                            let modal_state = modal_state.clone();
                            named_args!(scope.launch(async move {
                                if new_target {
                                    modal_state.expand().await;
                                } else {
                                    modal_state.collapse().await;
                                }
                            }));
                        }
                    })
                    .await
                });
            }

            // Display a modal container when expanded.
            if !is_collapsed.value() {
                let scope = remember_coroutine_scope();
                let predictive_back_progress: Animatable<f32, AnimationVector1D> = remember(|| Animatable(0.0f32));
                let predictive_back_state: Rc<RailPredictiveBackState> =
                    remember(|| Rc::new(RailPredictiveBackState::new()));

                {
                    let (channel, state) = (channel.clone(), state.clone());
                    SideEffect(move || {
                        channel.try_send(state.target_value().is_expanded());
                    });
                }

                let (dismiss_scope, dismiss_state) = (scope.clone(), state.clone());
                let (back_scope, back_progress) = (scope.clone(), predictive_back_progress.clone());
                let (cancel_scope, cancel_progress) = (scope.clone(), predictive_back_progress.clone());
                ModalWideNavigationRailDialog(
                    Rc::new(move || {
                        let state = dismiss_state.clone();
                        named_args!(dismiss_scope.launch(async move { state.collapse().await }));
                    }),
                    expanded_properties,
                    Rc::new(move |back_event: f32| {
                        let progress = back_progress.clone();
                        named_args!(back_scope.launch(async move { progress.snap_to(back_event).await }));
                    }),
                    Rc::new(move || {
                        let progress = cancel_progress.clone();
                        named_args!(cancel_scope.launch(async move {
                            named_args!(progress.animate_to(0.0)).await;
                        }));
                    }),
                    predictive_back_state.clone(),
                    Rc::new(move || {
                        let key_state = state.clone();
                        let key_scope = scope.clone();
                        let (modal_state, animate_to_dismiss, modal_animate_to_dismiss) =
                            (modal_state.clone(), animate_to_dismiss.clone(), modal_animate_to_dismiss.clone());
                        let (header, remember_content, modal_expanded) =
                            (header.clone(), remember_content.clone(), modal_expanded.clone());
                        let (predictive_back_progress, predictive_back_state) =
                            (predictive_back_progress.clone(), predictive_back_state.clone());
                        let (modifier, expanded_shape, window_insets, arrangement, content_padding) = (
                            modifier.clone(),
                            expanded_shape.clone(),
                            window_insets.clone(),
                            arrangement.clone(),
                            content_padding.clone(),
                        );
                        Box(
                            modifier = Modifier.fill_max_size().ime_padding().on_key_event(move |event| {
                                if event.r#type == KeyEventType::KeyUp && event.key == Key::Escape {
                                    let state = key_state.clone();
                                    named_args!(key_scope.launch(async move { state.collapse().await }));
                                    return true;
                                }
                                false
                            }),
                            content = move || {
                                let is_scrim_visible = if should_hide_on_collapse {
                                    modal_state.target_value() != WideNavigationRailValue::Collapsed
                                } else {
                                    modal_expanded.value()
                                };

                                let alpha = animate_float_as_state(
                                    if is_scrim_visible { 1.0 } else { 0.0 },
                                    // TODO: Load the motionScheme tokens from the component tokens file.
                                    animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>(),
                                );
                                Scrim(
                                    content_description = Some(get_string(Strings::CloseRail)),
                                    on_click = Some(animate_to_dismiss.clone()),
                                    alpha = Rc::new(move || alpha.value()) as Rc<dyn Fn() -> f32>,
                                    color = colors.modal_scrim_color,
                                );
                                let header = header.clone();
                                ModalWideNavigationRailContent(
                                    should_hide_on_collapse || modal_expanded.value(),
                                    should_hide_on_collapse,
                                    predictive_back_progress.clone(),
                                    predictive_back_state.clone(),
                                    modal_animate_to_dismiss.clone(),
                                    modifier.clone(),
                                    modal_state.clone(),
                                    colors,
                                    expanded_shape.clone(),
                                    ExpandedRailMaxWidth,
                                    Some(Rc::new(move || {
                                        let header = header.clone();
                                        Box(
                                            modifier =
                                                named_args!(Modifier.padding_sides(top = expanded_header_top_padding)),
                                            content = move || {
                                                if let Some(header) = &header {
                                                    header();
                                                }
                                            },
                                        );
                                    }) as Rc<dyn Fn()>),
                                    window_insets.clone(),
                                    should_hide_on_collapse,
                                    arrangement.clone(),
                                    content_padding.clone(),
                                    remember_content.clone(),
                                );
                            },
                        );
                    }),
                );
            }
        })
    }
}

/// The arguments of a [`ModalWideNavigationRail`], given to its override (Compose's experimental
/// `ModalWideNavigationRailOverrideScope`). Made by [`ModalWideNavigationRail`].
#[derive(Clone)]
pub struct ModalWideNavigationRailOverrideScope {
    modifier: Modifier,
    state: Rc<dyn WideNavigationRailState>,
    should_hide_on_collapse: bool,
    collapsed_shape: Arc<dyn Shape>,
    expanded_shape: Arc<dyn Shape>,
    colors: WideNavigationRailColors,
    header: Option<Rc<dyn Fn()>>,
    expanded_header_top_padding: Dp,
    window_insets: WindowInsets,
    arrangement: VerticalArrangement,
    expanded_properties: ModalWideNavigationRailProperties,
    content_padding: PaddingValues,
    content: Rc<dyn Fn()>,
}

impl ModalWideNavigationRailOverrideScope {
    /// The rail's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The rail's state (Compose's `state`).
    pub fn state(&self) -> Rc<dyn WideNavigationRailState> {
        self.state.clone()
    }

    /// Whether the rail slides off screen as it collapses (Compose's `shouldHideOnCollapse`).
    pub fn should_hide_on_collapse(&self) -> bool {
        self.should_hide_on_collapse
    }

    /// The collapsed rail's shape (Compose's `collapsedShape`).
    pub fn collapsed_shape(&self) -> Arc<dyn Shape> {
        self.collapsed_shape.clone()
    }

    /// The expanded rail's shape (Compose's `expandedShape`).
    pub fn expanded_shape(&self) -> Arc<dyn Shape> {
        self.expanded_shape.clone()
    }

    /// The rail's colors (Compose's `colors`).
    pub fn colors(&self) -> WideNavigationRailColors {
        self.colors
    }

    /// The rail's header, if any (Compose's `header`).
    pub fn header(&self) -> Option<Rc<dyn Fn()>> {
        self.header.clone()
    }

    /// The padding above the expanded rail's header (Compose's `expandedHeaderTopPadding`).
    pub fn expanded_header_top_padding(&self) -> Dp {
        self.expanded_header_top_padding
    }

    /// The insets the rail keeps clear of (Compose's `windowInsets`).
    pub fn window_insets(&self) -> WindowInsets {
        self.window_insets.clone()
    }

    /// How the rail arranges its items (Compose's `arrangement`).
    pub fn arrangement(&self) -> VerticalArrangement {
        self.arrangement.clone()
    }

    /// What the expanded rail's window does (Compose's `expandedProperties`).
    pub fn expanded_properties(&self) -> ModalWideNavigationRailProperties {
        self.expanded_properties
    }

    /// The padding inside the rail (Compose's `contentPadding`).
    pub fn content_padding(&self) -> PaddingValues {
        self.content_padding.clone()
    }

    /// The rail's items (Compose's `content`).
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the modal wide navigation rails below (Compose's experimental
/// `LocalModalWideNavigationRailOverride`): [`DefaultModalWideNavigationRailOverride`] unless provided.
pub static LocalModalWideNavigationRailOverride: KompozitionLocal<Rc<dyn ModalWideNavigationRailOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultModalWideNavigationRailOverride));

/// What the expanded modal wide navigation rail's window does (Compose's `ModalWideNavigationRailProperties`,
/// Compose for desktop's actual): whether back dismisses it. (Compose for desktop's `equals` compares it with a
/// `ModalBottomSheetProperties`, a slip that makes two never equal; here equal by what they hold.)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ModalWideNavigationRailProperties {
    pub should_dismiss_on_back_press: bool,
}

impl Default for ModalWideNavigationRailProperties {
    fn default() -> Self {
        ModalWideNavigationRailProperties { should_dismiss_on_back_press: true }
    }
}

/// Compose's `ModalWideNavigationRailProperties(shouldDismissOnBackPress = true)`.
#[named(type)]
pub fn ModalWideNavigationRailProperties(
    #[default(true)] should_dismiss_on_back_press: bool,
) -> ModalWideNavigationRailProperties {
    ModalWideNavigationRailProperties { should_dismiss_on_back_press }
}

/// The default properties (Compose's internal `createDefaultModalWideNavigationRailProperties`, Compose for
/// desktop's actual): dismissed by back.
const fn create_default_modal_wide_navigation_rail_properties() -> ModalWideNavigationRailProperties {
    ModalWideNavigationRailProperties { should_dismiss_on_back_press: true }
}

/// The expanded modal rail's window (Compose's internal `ModalWideNavigationRailDialog`, Compose for desktop's
/// actual): a [`Dialog`] dismissed by back as `properties` say, not of the platform's width nor within its insets
/// — of no scrim and no transition (KompozR's [`Dialog`] has none of its own). Compose for desktop's actual leaves
/// out the predictive back's callbacks and state (the desktop has no back gesture).
#[kompozable]
fn ModalWideNavigationRailDialog(
    on_dismiss_request: Rc<dyn Fn()>,
    properties: ModalWideNavigationRailProperties,
    _on_predictive_back: Rc<dyn Fn(f32)>,
    _on_predictive_back_cancelled: Rc<dyn Fn()>,
    _predictive_back_state: Rc<RailPredictiveBackState>,
    content: Rc<dyn Fn()>,
) {
    Dialog(
        on_dismiss_request = move || on_dismiss_request(),
        properties = DialogProperties {
            dismiss_on_back_press: properties.should_dismiss_on_back_press,
            dismiss_on_click_outside: true,
            use_platform_default_width: false,
            // (Compose for desktop's `usePlatformInsets = false`)
            decor_fits_system_windows: false,
        },
        content = move || content(),
    );
}

/// The expanded modal rail (Compose's private `ModalWideNavigationRailContent`): a [`Surface`] of the modal colors
/// up to `open_modal_rail_max_width` wide, as tall as it may be, a pane titled "Navigation rail" (in the platform's
/// language) to accessibility, anchored and dragged along `rail_state` (from off screen for a standalone modal,
/// swipes taken while `gestures_enabled`; a fling settling it, then the dismissing), scaled by the predictive back's
/// progress; holding an expanded (or not) wide rail of its own.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn ModalWideNavigationRailContent(
    expanded: bool,
    is_standalone_modal: bool,
    predictive_back_progress: Animatable<f32, AnimationVector1D>,
    predictive_back_state: Rc<RailPredictiveBackState>,
    modal_animate_to_dismiss: Rc<dyn Fn() -> LocalFuture<'static, ()>>,
    modifier: Modifier,
    rail_state: Rc<ModalWideNavigationRailState>,
    colors: WideNavigationRailColors,
    shape: Arc<dyn Shape>,
    open_modal_rail_max_width: Dp,
    header: Option<Rc<dyn Fn()>>,
    window_insets: WindowInsets,
    gestures_enabled: bool,
    arrangement: VerticalArrangement,
    content_padding: PaddingValues,
    content: Rc<dyn Fn()>,
) {
    let is_rtl = LocalLayoutDirection.current() == LayoutDirection::Rtl;
    let rail_pane_title = get_string(Strings::WideNavigationRailPaneTitle);
    let anchored_draggable_fling_behavior = AnchoredDraggableDefaults::fling_behavior(
        rail_state.anchored_draggable_state.clone(),
        Some(Rc::new(|distance: f32| distance * 0.5) as Rc<dyn Fn(f32) -> f32>),
        Some(rail_state.animation_spec.clone()),
    );

    let rail_fling_behavior: Rc<dyn FlingBehavior> =
        remember_keyed(Rc::as_ptr(&anchored_draggable_fling_behavior) as *const () as usize, || {
            Rc::new(RailFlingBehavior {
                anchored_draggable_fling_behavior: anchored_draggable_fling_behavior.clone(),
                modal_animate_to_dismiss: modal_animate_to_dismiss.clone(),
            }) as Rc<dyn FlingBehavior>
        });

    let (layer_progress, layer_back_state, layer_rail_state) =
        (predictive_back_progress.clone(), predictive_back_state.clone(), rail_state.clone());
    let anchors_rail_state = rail_state.clone();
    Surface(
        shape = shape.clone(),
        color = colors.modal_container_color,
        content_color = Some(colors.modal_content_color),
        modifier = named_args!(modifier.width_in(max = open_modal_rail_max_width))
            .fill_max_height()
            .semantics_with(false, move |c| c.set_pane_title(rail_pane_title.clone()))
            .graphics_layer_block(move |scope| {
                let progress = layer_progress.value();
                if progress <= 0.0 {
                    return;
                }
                let offset = layer_rail_state.current_offset();
                let width = scope.size().width;
                if !offset.is_nan() && !width.is_nan() && width != 0.0 {
                    // Apply the predictive back animation.
                    scope.scale_x =
                        calculate_predictive_back_scale_x(scope, progress, layer_back_state.swipe_edge_matches_rail());
                    scope.scale_y = calculate_predictive_back_scale_y(scope, progress);
                    scope.transform_origin =
                        TransformOrigin::new(if is_rtl { 1.0 } else { 0.0 }, PredictiveBackPivotFractionY);
                }
            })
            .draggable_anchors(
                rail_state.anchored_draggable_state.clone(),
                Orientation::Horizontal,
                move |rail_size, _| {
                    let width = rail_size.width as f32;
                    let min_value = if is_standalone_modal { -width } else { 0.0 };
                    let max_value = 0.0;
                    let anchors = DraggableAnchors(|c: &mut DraggableAnchorsConfig<WideNavigationRailValue>| {
                        c.at(WideNavigationRailValue::Collapsed, min_value);
                        c.at(WideNavigationRailValue::Expanded, max_value);
                    });
                    (
                        Arc::new(anchors) as Arc<dyn kompoz_foundation::gestures::DraggableAnchors<_>>,
                        anchors_rail_state.target_value(),
                    )
                },
            )
            .anchored_draggable(
                rail_state.anchored_draggable_state.clone(),
                Orientation::Horizontal,
                gestures_enabled,
                None,
                None,
                Some(rail_fling_behavior),
            ),
        content = || {
            let (progress, back_state) = (predictive_back_progress.clone(), predictive_back_state.clone());
            WideNavigationRailLayout(
                Modifier.graphics_layer_block(move |scope| {
                    let progress = progress.value();
                    if progress <= 0.0 {
                        return;
                    }
                    // Preserve the original aspect ratio and alignment due to the predictive back
                    // animation.
                    let predictive_back_scale_x =
                        calculate_predictive_back_scale_x(scope, progress, back_state.swipe_edge_matches_rail());
                    let predictive_back_scale_y = calculate_predictive_back_scale_y(scope, progress);
                    scope.scale_x = if predictive_back_scale_x != 0.0 {
                        predictive_back_scale_y / predictive_back_scale_x
                    } else {
                        1.0
                    };
                    scope.transform_origin =
                        TransformOrigin::new(if is_rtl { 0.0 } else { 1.0 }, PredictiveBackPivotFractionY);
                }),
                true,
                expanded,
                colors,
                shape.clone(),
                header.clone(),
                window_insets.clone(),
                arrangement.clone(),
                content_padding.clone(),
                content.clone(),
            );
        },
    );
}

/// The modal rail's fling (Compose's anonymous `FlingBehavior` in `ModalWideNavigationRailContent`): the anchored
/// dragging's fling, then the dismissing (in Compose's `finally`). If the fling is cancelled the dismissing isn't
/// run: Compose's runs it in the cancelled coroutine, where its animation is cancelled as it starts.
struct RailFlingBehavior {
    anchored_draggable_fling_behavior: Rc<dyn TargetedFlingBehavior>,
    modal_animate_to_dismiss: Rc<dyn Fn() -> LocalFuture<'static, ()>>,
}

impl PartialEq for RailFlingBehavior {
    /// The same object (Kotlin's anonymous object: equal by identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl FlingBehavior for RailFlingBehavior {
    fn perform_fling(&self, scope: Rc<dyn ScrollScope>, initial_velocity: f32) -> LocalFuture<'_, f32> {
        Box::pin(async move {
            let remaining_velocity =
                self.anchored_draggable_fling_behavior.perform_fling(scope, initial_velocity).await;
            (self.modal_animate_to_dismiss)().await;
            remaining_velocity
        })
    }
}

/// The predictive back's horizontal scale (Compose's private `GraphicsLayerScope.calculatePredictiveBackScaleX`):
/// up to 24 dp wider (narrower if the swipe isn't from the rail's edge) as `progress` goes.
fn calculate_predictive_back_scale_x(scope: &GraphicsLayerScope, progress: f32, swipe_edge_matches_rail: bool) -> f32 {
    let width = scope.size().width;
    if width.is_nan() || width == 0.0 {
        1.0
    } else {
        let scale_x_direction = if swipe_edge_matches_rail { 1.0 } else { -1.0 };
        1.0 + (scale_x_direction
            * lerp(0.0, scope.density().to_px(PredictiveBackMaxScaleXDistance).min(width), progress))
            / width
    }
}

/// The predictive back's vertical scale (Compose's private `GraphicsLayerScope.calculatePredictiveBackScaleY`): up
/// to 48 dp shorter as `progress` goes.
fn calculate_predictive_back_scale_y(scope: &GraphicsLayerScope, progress: f32) -> f32 {
    let height = scope.size().height;
    if height.is_nan() || height == 0.0 {
        1.0
    } else {
        1.0 - lerp(0.0, scope.density().to_px(PredictiveBackMaxScaleYDistance).min(height), progress) / height
    }
}

// ---- WideNavigationRailItem -------------------------------------------------------------------------------------

/// A destination of a wide navigation rail (Compose's `WideNavigationRailItem`): an animated navigation item —
/// its icon over its label (`icon_position` Top, a collapsed rail's) or before it (Start, an expanded rail's), moving
/// between the two as `icon_position` changes; the indicator (round, in `colors`' selected indicator color) behind
/// the icon (or both) growing in as it is picked; the whole item selectable (the Tab role) calling `on_click`
/// (not when `enabled` is false). `icon_position` None is Kotlin's default,
/// [`WideNavigationRailItemDefaults::icon_position_for`]`(rail_expanded)`; `indicator_padding` None is Kotlin's
/// default, [`WideNavigationRailItemDefaults::indicator_padding`]`(rail_expanded)` (padding that moves with the
/// icon). Its interactions go to `interaction_source` (one of its own where none).
///
/// Kotlin: `WideNavigationRailItem(selected: Boolean, onClick: () -> Unit, icon: @Composable () -> Unit, label: @Composable (() -> Unit)?, railExpanded: Boolean, modifier: Modifier = Modifier, enabled: Boolean = true, iconPosition: NavigationItemIconPosition = WideNavigationRailItemDefaults.iconPositionFor(railExpanded), colors: NavigationItemColors = WideNavigationRailItemDefaults.colors(), interactionSource: MutableInteractionSource? = null, indicatorPadding: PaddingValues = WideNavigationRailItemDefaults.indicatorPadding(railExpanded = railExpanded))`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn WideNavigationRailItem(
    selected: bool,
    on_click: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    label: Option<Rc<dyn Fn()>>,
    rail_expanded: bool,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] icon_position: Option<NavigationItemIconPosition>,
    #[default(WideNavigationRailItemDefaults.colors())] colors: NavigationItemColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
    #[default(None)] indicator_padding: Option<PaddingValues>,
) {
    let icon_position =
        icon_position.unwrap_or_else(|| WideNavigationRailItemDefaults::icon_position_for(rail_expanded));
    let indicator_padding = match indicator_padding {
        Some(indicator_padding) => indicator_padding,
        None => named_args!(WideNavigationRailItemDefaults.indicator_padding(rail_expanded = rail_expanded)),
    };
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));

    AnimatedNavigationItem(
        selected,
        Rc::new(on_click),
        Rc::new(icon),
        NavigationRailBaselineItemTokens::ActiveIndicatorShape.value(),
        NavigationRailVerticalItemTokens::ActiveIndicatorWidth,
        NavigationRailVerticalItemTokens::LabelTextFont.value(),
        NavigationRailHorizontalItemTokens::LabelTextFont.value(),
        indicator_padding,
        NavigationRailVerticalItemTokens::IconLabelSpace,
        WNRItemNoLabelIndicatorPadding,
        NavigationRailHorizontalItemTokens::IconLabelSpace,
        WNRItemHorizontalPadding,
        colors,
        modifier,
        enabled,
        label,
        icon_position,
        interaction_source,
    );
}

// ---- Colors and defaults ----------------------------------------------------------------------------------------

/// A wide navigation rail's colors (Compose's `WideNavigationRailColors`): its container's and content's, and the
/// modal rail's container, scrim and content. Equal as Compose's `equals` says: by all but the modal content color.
#[derive(Clone, Copy, Debug, KotlinProperties)]
pub struct WideNavigationRailColors {
    /// The rail's background.
    pub container_color: Color,
    /// The color of the rail's content.
    pub content_color: Color,
    /// The expanded modal rail's background.
    pub modal_container_color: Color,
    /// The scrim behind the expanded modal rail.
    pub modal_scrim_color: Color,
    /// The color of the expanded modal rail's content.
    pub modal_content_color: Color,
}

impl PartialEq for WideNavigationRailColors {
    /// (Compose's `equals` leaves the modal content color out, its `hashCode` not.)
    fn eq(&self, other: &Self) -> bool {
        self.container_color == other.container_color
            && self.content_color == other.content_color
            && self.modal_container_color == other.modal_container_color
            && self.modal_scrim_color == other.modal_scrim_color
    }
}

impl WideNavigationRailColors {
    /// Colors of one's own (Compose's constructor).
    ///
    /// Kotlin: `WideNavigationRailColors(containerColor: Color, contentColor: Color, modalContainerColor: Color, modalScrimColor: Color, modalContentColor: Color)`
    pub fn new(
        container_color: Color,
        content_color: Color,
        modal_container_color: Color,
        modal_scrim_color: Color,
        modal_content_color: Color,
    ) -> WideNavigationRailColors {
        WideNavigationRailColors {
            container_color,
            content_color,
            modal_container_color,
            modal_scrim_color,
            modal_content_color,
        }
    }

    /// Colors of one's own, the modal rail's content in `content_color` (Compose's deprecated constructor without
    /// `modalContentColor`).
    ///
    /// Kotlin: `WideNavigationRailColors(containerColor: Color, contentColor: Color, modalContainerColor: Color, modalScrimColor: Color)`
    #[deprecated(note = "Deprecated in favor of constructor with modalContentColor parameter")]
    pub fn without_modal_content_color(
        container_color: Color,
        content_color: Color,
        modal_container_color: Color,
        modal_scrim_color: Color,
    ) -> WideNavigationRailColors {
        WideNavigationRailColors::new(
            container_color,
            content_color,
            modal_container_color,
            modal_scrim_color,
            content_color,
        )
    }
}

#[named]
impl WideNavigationRailColors {
    /// These colors with those given in place of their own (Compose's `copy`): an unspecified one keeps its own.
    pub fn copy(
        &self,
        #[default(self.container_color)] container_color: Color,
        #[default(self.content_color)] content_color: Color,
        #[default(self.modal_container_color)] modal_container_color: Color,
        #[default(self.modal_scrim_color)] modal_scrim_color: Color,
        #[default(self.modal_content_color)] modal_content_color: Color,
    ) -> WideNavigationRailColors {
        WideNavigationRailColors::new(
            container_color.take_or_else(|| self.container_color),
            content_color.take_or_else(|| self.content_color),
            modal_container_color.take_or_else(|| self.modal_container_color),
            modal_scrim_color.take_or_else(|| self.modal_scrim_color),
            modal_content_color.take_or_else(|| self.modal_content_color),
        )
    }
}

/// The wide navigation rails' defaults (Compose's `WideNavigationRailDefaults`), used as a value
/// (`WideNavigationRailDefaults.colors()`).
pub struct WideNavigationRailDefaults;

impl WideNavigationRailDefaults {
    /// A wide navigation rail's shape: square corners (Compose's `shape`).
    pub fn shape() -> Arc<dyn Shape> {
        NavigationRailCollapsedTokens::ContainerShape.value()
    }

    /// How a wide navigation rail arranges its items: from the top (Compose's `arrangement`).
    pub fn arrangement() -> VerticalArrangement {
        Arrangement::Top
    }

    /// The insets a wide navigation rail keeps clear of: the system bars' at the top, the bottom and the start
    /// (Compose's `windowInsets`).
    #[track_caller]
    pub fn window_insets() -> WindowInsets {
        system_bars_for_visual_components().only(WindowInsetsSides::Vertical + WindowInsetsSides::Start)
    }

    /// The collapsed modal rail's shape: the rail's (Compose's `modalCollapsedShape`).
    pub fn modal_collapsed_shape() -> Arc<dyn Shape> {
        WideNavigationRailDefaults::shape()
    }

    /// The expanded modal rail's shape: the theme's large corners at its end (Compose's `modalExpandedShape`).
    pub fn modal_expanded_shape() -> Arc<dyn Shape> {
        NavigationRailExpandedTokens::ModalContainerShape.value()
    }

    /// What the expanded modal rail's window does by default: dismissed by back (Compose's
    /// `ModalExpandedProperties`).
    pub const ModalExpandedProperties: ModalWideNavigationRailProperties =
        create_default_modal_wide_navigation_rail_properties();

    /// The padding inside a wide navigation rail: 44 dp at the top and the bottom (Compose's `ContentPadding`).
    pub fn ContentPadding() -> PaddingValues {
        PaddingValues::new(Dp(0.0), WNRVerticalPadding, Dp(0.0), WNRVerticalPadding)
    }

    /// The deprecated shape of a wide navigation rail (Compose's deprecated `containerShape`): [`shape`](Self::shape).
    #[deprecated(note = "Deprecated in favor of shape.")]
    pub fn container_shape() -> Arc<dyn Shape> {
        NavigationRailCollapsedTokens::ContainerShape.value()
    }

    /// The deprecated shape of the expanded modal rail (Compose's deprecated `modalContainerShape`):
    /// [`modal_expanded_shape`](Self::modal_expanded_shape).
    #[deprecated(note = "Deprecated in favor of modalExpandedShape.")]
    pub fn modal_container_shape() -> Arc<dyn Shape> {
        NavigationRailExpandedTokens::ModalContainerShape.value()
    }

    /// A rail's background: the scheme's surface (Compose's private `containerColor`).
    fn container_color() -> Color {
        NavigationRailCollapsedTokens::ContainerColor.value()
    }

    /// The expanded modal rail's background: the scheme's surface container (Compose's private
    /// `modalContainerColor`).
    fn modal_container_color() -> Color {
        NavigationRailExpandedTokens::ModalContainerColor.value()
    }

    /// The theme's rail colors (Compose's private `ColorScheme.defaultWideWideNavigationRailColors`; made afresh, as
    /// KompozR keeps no per-scheme caches).
    fn default_wide_wide_navigation_rail_colors(_scheme: &ColorScheme) -> WideNavigationRailColors {
        let container_color = WideNavigationRailDefaults::container_color();
        let modal_container_color = WideNavigationRailDefaults::modal_container_color();
        WideNavigationRailColors {
            container_color,
            content_color: content_color_for(container_color),
            modal_container_color,
            modal_content_color: content_color_for(modal_container_color),
            modal_scrim_color: named_args!(
                ScrimTokens::ContainerColor.value().copy(alpha = ScrimTokens::ContainerOpacity)
            ),
        }
    }
}

#[named]
impl WideNavigationRailDefaults {
    /// A wide navigation rail's colors (Compose's `WideNavigationRailDefaults.colors()` and `colors(containerColor,
    /// …)`): by default the theme's — the surface and its content color, the expanded modal rail on the surface
    /// container, the scrim at 32%. `content_color` None is Kotlin's default, `content_color_for(container_color)`;
    /// `modal_content_color` None, `content_color_for(modal_container_color)`.
    ///
    /// Kotlin: `colors(containerColor: Color = WideNavigationRailDefaults.containerColor, contentColor: Color = contentColorFor(containerColor), modalContainerColor: Color = NavigationRailExpandedTokens.ModalContainerColor.value, modalScrimColor: Color = ScrimTokens.ContainerColor.value.copy(ScrimTokens.ContainerOpacity), modalContentColor: Color = contentColorFor(modalContainerColor)): WideNavigationRailColors`
    pub fn colors(
        &self,
        #[default(WideNavigationRailDefaults::container_color())] container_color: Color,
        #[default(None)] content_color: Option<Color>,
        #[default(NavigationRailExpandedTokens::ModalContainerColor.value())] modal_container_color: Color,
        #[default(named_args!(ScrimTokens::ContainerColor.value().copy(alpha = ScrimTokens::ContainerOpacity)))]
        modal_scrim_color: Color,
        #[default(None)] modal_content_color: Option<Color>,
    ) -> WideNavigationRailColors {
        let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
        let modal_content_color = modal_content_color.unwrap_or_else(|| content_color_for(modal_container_color));
        named_args!(
            WideNavigationRailDefaults::default_wide_wide_navigation_rail_colors(&MaterialTheme::color_scheme()).copy(
                container_color = container_color,
                content_color = content_color,
                modal_container_color = modal_container_color,
                modal_scrim_color = modal_scrim_color,
                modal_content_color = modal_content_color,
            )
        )
    }
}

/// The wide navigation rail items' defaults (Compose's `WideNavigationRailItemDefaults`), used as a value
/// (`WideNavigationRailItemDefaults.colors()`).
pub struct WideNavigationRailItemDefaults;

impl WideNavigationRailItemDefaults {
    /// Where an item's icon goes: before its label in an expanded rail, over it in a collapsed one (Compose's
    /// `iconPositionFor`).
    ///
    /// Kotlin: `iconPositionFor(railExpanded: Boolean): NavigationItemIconPosition`
    pub fn icon_position_for(rail_expanded: bool) -> NavigationItemIconPosition {
        if rail_expanded { NavigationItemIconPosition::Start } else { NavigationItemIconPosition::Top }
    }

    /// The indicator's padding round a top icon: 16 dp at the sides, 4 dp above and below (Compose's
    /// `IndicatorCollapsedPadding`).
    pub fn IndicatorCollapsedPadding() -> PaddingValues {
        PaddingValues::hv(ItemTopIconIndicatorHorizontalPadding, ItemTopIconIndicatorVerticalPadding)
    }

    /// The indicator's padding round a start icon and its label: 16 dp at the sides, 16 dp above and below
    /// (Compose's `IndicatorExpandedPadding`).
    pub fn IndicatorExpandedPadding() -> PaddingValues {
        PaddingValues::hv(
            NavigationRailHorizontalItemTokens::FullWidthLeadingSpace,
            ItemStartIconIndicatorVerticalPadding,
        )
    }

    /// The theme's item colors (Compose's private `ColorScheme.defaultWideNavigationRailItemColors`; made afresh).
    fn default_wide_navigation_rail_item_colors(scheme: &ColorScheme) -> NavigationItemColors {
        NavigationItemColors::new(
            scheme.from_token(NavigationRailColorTokens::ItemActiveIcon),
            scheme.from_token(NavigationRailColorTokens::ItemActiveLabelText),
            scheme.from_token(NavigationRailColorTokens::ItemActiveIndicator),
            scheme.from_token(NavigationRailColorTokens::ItemInactiveIcon),
            scheme.from_token(NavigationRailColorTokens::ItemInactiveLabelText),
            named_args!(scheme.from_token(NavigationRailColorTokens::ItemInactiveIcon).copy(alpha = DisabledAlpha)),
            named_args!(
                scheme.from_token(NavigationRailColorTokens::ItemInactiveLabelText).copy(alpha = DisabledAlpha)
            ),
        )
    }
}

#[named]
impl WideNavigationRailItemDefaults {
    /// The indicator's padding of an item of a rail `rail_expanded` or not: padding that goes from
    /// `collapsed_padding` to `expanded_padding` as the item's icon moves (Compose's `indicatorPadding`; remembered
    /// for its arguments).
    ///
    /// Kotlin: `indicatorPadding(railExpanded: Boolean, collapsedPadding: PaddingValues = IndicatorCollapsedPadding, expandedPadding: PaddingValues = IndicatorExpandedPadding): PaddingValues`
    #[track_caller]
    pub fn indicator_padding(
        &self,
        rail_expanded: bool,
        #[default(WideNavigationRailItemDefaults::IndicatorCollapsedPadding())] collapsed_padding: PaddingValues,
        #[default(WideNavigationRailItemDefaults::IndicatorExpandedPadding())] expanded_padding: PaddingValues,
    ) -> PaddingValues {
        kompozable_block!(remember_keyed((collapsed_padding.clone(), expanded_padding.clone(), rail_expanded), || {
            PaddingValues::custom(DynamicPaddingValues::new(
                collapsed_padding.clone(),
                expanded_padding.clone(),
                rail_expanded,
            ))
        }))
    }

    /// A wide navigation rail item's colors (Compose's `WideNavigationRailItemDefaults.colors()` and
    /// `colors(selectedIconColor, …)`): by default the scheme's — selected, the icon in on-secondary-container,
    /// the label in secondary, the indicator in secondary container; unselected, both in on-surface-variant;
    /// disabled, the unselected ones at 38% (None is Kotlin's default for `disabled_icon_color` and
    /// `disabled_text_color`: `unselected_icon_color` / `unselected_text_color` at 38%).
    ///
    /// Kotlin: `colors(selectedIconColor: Color = NavigationRailColorTokens.ItemActiveIcon.value, selectedTextColor: Color = NavigationRailColorTokens.ItemActiveLabelText.value, selectedIndicatorColor: Color = NavigationRailColorTokens.ItemActiveIndicator.value, unselectedIconColor: Color = NavigationRailColorTokens.ItemInactiveIcon.value, unselectedTextColor: Color = NavigationRailColorTokens.ItemInactiveLabelText.value, disabledIconColor: Color = unselectedIconColor.copy(alpha = DisabledAlpha), disabledTextColor: Color = unselectedTextColor.copy(alpha = DisabledAlpha)): NavigationItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(NavigationRailColorTokens::ItemActiveIcon.value())] selected_icon_color: Color,
        #[default(NavigationRailColorTokens::ItemActiveLabelText.value())] selected_text_color: Color,
        #[default(NavigationRailColorTokens::ItemActiveIndicator.value())] selected_indicator_color: Color,
        #[default(NavigationRailColorTokens::ItemInactiveIcon.value())] unselected_icon_color: Color,
        #[default(NavigationRailColorTokens::ItemInactiveLabelText.value())] unselected_text_color: Color,
        #[default(None)] disabled_icon_color: Option<Color>,
        #[default(None)] disabled_text_color: Option<Color>,
    ) -> NavigationItemColors {
        let disabled_icon_color =
            disabled_icon_color.unwrap_or_else(|| named_args!(unselected_icon_color.copy(alpha = DisabledAlpha)));
        let disabled_text_color =
            disabled_text_color.unwrap_or_else(|| named_args!(unselected_text_color.copy(alpha = DisabledAlpha)));
        named_args!(
            WideNavigationRailItemDefaults::default_wide_navigation_rail_item_colors(&MaterialTheme::color_scheme())
                .copy(
                    selected_icon_color = selected_icon_color,
                    selected_text_color = selected_text_color,
                    selected_indicator_color = selected_indicator_color,
                    unselected_icon_color = unselected_icon_color,
                    unselected_text_color = unselected_text_color,
                    disabled_icon_color = disabled_icon_color,
                    disabled_text_color = disabled_text_color,
                )
        )
    }
}

/// The modal wide navigation rail's deprecated defaults (Compose's deprecated `ModalWideNavigationRailDefaults`):
/// [`WideNavigationRailDefaults`]'s.
#[deprecated(note = "Deprecated in favor of default values in WideNavigationRailDefaults")]
pub struct ModalWideNavigationRailDefaults;

#[allow(deprecated)]
impl ModalWideNavigationRailDefaults {
    /// The expanded modal rail's properties (Compose's deprecated `Properties`):
    /// [`WideNavigationRailDefaults::ModalExpandedProperties`].
    #[deprecated(note = "Deprecated in favor of function with WideNavigationRailDefaults.ModalExpandedProperties")]
    pub const Properties: ModalWideNavigationRailProperties = create_default_modal_wide_navigation_rail_properties();
}

/// The indicator's padding at each side of a top icon without a label (Compose's internal
/// `WNRItemNoLabelIndicatorPadding`): 16 dp.
pub(crate) const WNRItemNoLabelIndicatorPadding: Dp =
    Dp((NavigationRailVerticalItemTokens::ActiveIndicatorWidth.0 - NavigationRailBaselineItemTokens::IconSize.0) / 2.0);

/// The padding at each side of an item (Compose's internal `WNRItemHorizontalPadding`): 20 dp.
pub(crate) const WNRItemHorizontalPadding: Dp = Dp(20.0);

// Vertical padding between the contents of the wide navigation rail and its top/bottom.
const WNRVerticalPadding: Dp = NavigationRailCollapsedTokens::TopSpace;
// Padding at the bottom of the rail's header. This padding will only be added when the header is
// not null and the rail arrangement is Top.
const WNRHeaderPadding: Dp = NavigationRailBaselineItemTokens::HeaderSpaceMinimum;
const CollapsedRailWidth: Dp = NavigationRailCollapsedTokens::ContainerWidth;
const ExpandedRailMinWidth: Dp = NavigationRailExpandedTokens::ContainerWidthMinimum;
const ExpandedRailMaxWidth: Dp = NavigationRailExpandedTokens::ContainerWidthMaximum;
const TopIconItemMinHeight: Dp = NavigationRailBaselineItemTokens::ContainerHeight;
const ItemTopIconIndicatorVerticalPadding: Dp = Dp((NavigationRailVerticalItemTokens::ActiveIndicatorHeight.0
    - NavigationRailBaselineItemTokens::IconSize.0)
    / 2.0);
const ItemTopIconIndicatorHorizontalPadding: Dp =
    Dp((NavigationRailVerticalItemTokens::ActiveIndicatorWidth.0 - NavigationRailBaselineItemTokens::IconSize.0) / 2.0);
const ItemStartIconIndicatorVerticalPadding: Dp = Dp((NavigationRailHorizontalItemTokens::ActiveIndicatorHeight.0
    - NavigationRailBaselineItemTokens::IconSize.0)
    / 2.0);
const PredictiveBackMaxScaleXDistance: Dp = Dp(24.0);
const PredictiveBackMaxScaleYDistance: Dp = Dp(48.0);

const PredictiveBackPivotFractionY: f32 = 0.5;
const HeaderLayoutIdTag: &str = "header";
