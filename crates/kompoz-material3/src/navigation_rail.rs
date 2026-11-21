//! Material's navigation rail, as Compose's NavigationRail.kt (material3 1.5.0-alpha17): a column of destinations
//! down the start of a wide screen ([`NavigationRail`], 80 dp wide, an optional header — a FAB or a logo — above
//! its items, clear of the system bars at the top, bottom and start), each a [`NavigationRailItem`]: an icon with an
//! optional label under it, a pill behind the icon growing in as the item is picked (the ripple in the pill, where
//! it was pressed), the colors of [`NavigationRailItemColors`]. How a rail is made can be replaced below a
//! [`LocalNavigationRailOverride`] (Compose's experimental override API).

use crate::badge::badge_bounds;
use crate::color_scheme::{ColorScheme, DisabledAlpha, content_color_for};
use crate::content_color::LocalContentColor;
use crate::internal::mapped_interaction_source::MappedInteractionSource;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::internal::shape_util::ClipShape;
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use crate::surface::Surface;
use crate::tokens::{
    MotionSchemeKeyTokens, NavigationRailBaselineItemTokens, NavigationRailCollapsedTokens, NavigationRailColorTokens,
    NavigationRailVerticalItemTokens, ShapeKeyTokens,
};
use kompoz_animation::{animate_color_as_state, animate_float_as_state};
use kompoz_foundation::interaction::{InteractionSource, MutableInteractionSource};
use kompoz_foundation::layout::{WindowInsets, WindowInsetsSides};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Constraints, Dp, Offset, Shape};
use kompoz_runtime::kt::KtFloat;
use kompoz_runtime::{
    KompozitionLocal, KompozitionLocalProvider, KotlinProperties, State, kompozable, kompozable_block, named,
    named_args, remember, remember_keyed,
};
use kompoz_ui::layout::{Layout, Measurable, MeasureResult, MeasureScope, Placeable, measure_policy};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;
use std::sync::Arc;

/// A navigation rail (Compose's `NavigationRail`): a [`Surface`] of `container_color` (the scheme's surface by
/// default) holding a column as tall as it may be, at least 80 dp wide, padded by `window_insets` (the system bars
/// at the top, bottom and start by default) and 4 dp at the top and bottom, its items centered and 4 dp apart —
/// `header` (if any) first, 8 dp more under it, then `content` (typically 3 to 7 [`NavigationRailItem`]s). The
/// column is a selectable group and a traversal group to accessibility. Content takes `content_color` (None is
/// Kotlin's default, `content_color_for(container_color)`). Made by [`LocalNavigationRailOverride`]'s override
/// ([`DefaultNavigationRailOverride`] unless provided).
///
/// Kotlin: `NavigationRail(modifier: Modifier = Modifier, containerColor: Color = NavigationRailDefaults.ContainerColor, contentColor: Color = contentColorFor(containerColor), header: @Composable (ColumnScope.() -> Unit)? = null, windowInsets: WindowInsets = NavigationRailDefaults.windowInsets, content: @Composable ColumnScope.() -> Unit)`
#[kompozable]
pub fn NavigationRail(
    #[default(Modifier)] modifier: Modifier,
    #[default(NavigationRailDefaults::ContainerColor())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(None)] header: Option<Rc<dyn Fn()>>,
    #[default(NavigationRailDefaults::window_insets())] window_insets: WindowInsets,
    content: impl Fn() + 'static,
) {
    let content_color = content_color.unwrap_or_else(|| content_color_for(container_color));
    let override_ = LocalNavigationRailOverride.current();
    override_.NavigationRail(&NavigationRailOverrideScope {
        modifier,
        container_color,
        content_color,
        header,
        window_insets,
        content: Rc::new(content),
    });
}

/// The navigation rail of an override (Compose's experimental `NavigationRailOverride`): what makes every
/// [`NavigationRail`] below a [`LocalNavigationRailOverride`] providing it, from the scope's arguments.
pub trait NavigationRailOverride {
    /// Composes the rail of `scope`'s arguments (Compose's `NavigationRailOverrideScope.NavigationRail()`).
    fn NavigationRail(&self, scope: &NavigationRailOverrideScope);
}

impl PartialEq for dyn NavigationRailOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default way a navigation rail is made (Compose's experimental `DefaultNavigationRailOverride`): a surface
/// holding the header and the items in a column, as [`NavigationRail`] says.
#[derive(Clone, Copy, Debug)]
pub struct DefaultNavigationRailOverride;

impl NavigationRailOverride for DefaultNavigationRailOverride {
    #[track_caller]
    fn NavigationRail(&self, scope: &NavigationRailOverrideScope) {
        let NavigationRailOverrideScope { modifier, container_color, content_color, header, window_insets, content } =
            scope.clone();
        kompozable_block!({
            Surface(
                color = container_color,
                content_color = Some(content_color),
                modifier = modifier,
                content = || {
                    let (header, content) = (header.clone(), content.clone());
                    Column(
                        modifier = named_args!(
                            Modifier
                                .fill_max_height()
                                .window_insets_padding(window_insets.clone())
                                .width_in(min = NavigationRailCollapsedTokens::NarrowContainerWidth)
                                .padding_hv(vertical = NavigationRailVerticalPadding)
                        )
                        .selectable_group()
                        .semantics_with(false, |c| c.set_is_traversal_group(true)),
                        horizontal_alignment = Alignment::CenterHorizontally,
                        vertical_arrangement = Arrangement::spaced_by(NavigationRailVerticalPadding),
                        content = move || {
                            if let Some(header) = header.clone() {
                                header();
                                Spacer(Modifier.height(NavigationRailHeaderPadding));
                            }
                            content();
                        },
                    );
                },
            );
        })
    }
}

/// The arguments of a [`NavigationRail`], given to its override (Compose's experimental
/// `NavigationRailOverrideScope`). Made by [`NavigationRail`].
#[derive(Clone)]
pub struct NavigationRailOverrideScope {
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
    header: Option<Rc<dyn Fn()>>,
    window_insets: WindowInsets,
    content: Rc<dyn Fn()>,
}

impl NavigationRailOverrideScope {
    /// The rail's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The rail's background (Compose's `containerColor`).
    pub fn container_color(&self) -> Color {
        self.container_color
    }

    /// The color of the rail's content (Compose's `contentColor`).
    pub fn content_color(&self) -> Color {
        self.content_color
    }

    /// The rail's header, if any (Compose's `header`).
    pub fn header(&self) -> Option<Rc<dyn Fn()>> {
        self.header.clone()
    }

    /// The insets the rail keeps clear of (Compose's `windowInsets`).
    pub fn window_insets(&self) -> WindowInsets {
        self.window_insets.clone()
    }

    /// The rail's items (Compose's `content`).
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the navigation rails below (Compose's experimental `LocalNavigationRailOverride`):
/// [`DefaultNavigationRailOverride`] unless provided.
pub static LocalNavigationRailOverride: KompozitionLocal<Rc<dyn NavigationRailOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultNavigationRailOverride));

/// A destination of a [`NavigationRail`] (Compose's `NavigationRailItem`): `icon` with `label` (if any) under it,
/// at least 80 × 56 dp, the whole item selectable (the Tab role) calling `on_click` (not when `enabled` is false).
/// A pill of the indicator color grows from the middle behind the icon as the item is picked (56 × 32 dp with a
/// label, 56 dp round without) and fades out as it is left; presses ripple in the pill from where they were. The
/// label shows always, or with `always_show_label` false only while selected (the icon moving up to make room).
/// Icon and label take `colors`' colors for the item's state, faded between them; a shown label stands for the
/// icon to accessibility (the icon's semantics cleared). Its interactions go to `interaction_source` (one of its own
/// where none). It gives a badge the bounds it keeps in.
///
/// Kotlin: `NavigationRailItem(selected: Boolean, onClick: () -> Unit, icon: @Composable () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, label: @Composable (() -> Unit)? = null, alwaysShowLabel: Boolean = true, colors: NavigationRailItemColors = NavigationRailItemDefaults.colors(), interactionSource: MutableInteractionSource? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavigationRailItem(
    selected: bool,
    on_click: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] label: Option<Rc<dyn Fn()>>,
    #[default(true)] always_show_label: bool,
    #[default(NavigationRailItemDefaults.colors())] colors: NavigationRailItemColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));
    // TODO Load the motionScheme tokens from the component tokens file
    let color_animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<Color>();
    let has_label = label.is_some();
    let icon: Rc<dyn Fn()> = Rc::new(icon);
    let styled_icon: Rc<dyn Fn()> = {
        let color_animation_spec = color_animation_spec.clone();
        Rc::new(move || {
            let icon_color = animate_color_as_state(
                colors.icon_color(selected, enabled),
                animation_spec = color_animation_spec.clone(),
            );
            // If there's a label, don't have a11y services repeat the icon description.
            let clear_semantics = has_label && (always_show_label || selected);
            let icon = icon.clone();
            Box(
                modifier = if clear_semantics { Modifier.clear_and_set_semantics(|_| {}) } else { Modifier },
                content = move || {
                    KompozitionLocalProvider(&[LocalContentColor.provides(icon_color.value())], {
                        let icon = icon.clone();
                        move || icon()
                    })
                },
            );
        })
    };

    let styled_label: Option<Rc<dyn Fn()>> = label.map(|label| {
        Rc::new(move || {
            let style = NavigationRailVerticalItemTokens::LabelTextFont.value();
            let text_color = animate_color_as_state(
                colors.text_color(selected, enabled),
                animation_spec = color_animation_spec.clone(),
            );
            let label = label.clone();
            ProvideContentColorTextStyle(text_color.value(), style, move || label());
        }) as Rc<dyn Fn()>
    });

    let on_click: Rc<dyn Fn()> = Rc::new(on_click);
    Box(
        modifier = modifier
            .selectable_indication_with(
                selected,
                Some(interaction_source.clone()),
                None,
                enabled,
                Some(Role::Tab),
                move || on_click(),
            )
            .default_min_size_with(Dp::Unspecified, NavigationRailItemHeight)
            .width_in_with(NavigationRailItemWidth, Dp::Unspecified),
        content_alignment = Alignment::Center,
        propagate_min_constraints = true,
        content = move || {
            let alpha_animation_progress: State<f32> = animate_float_as_state(
                if selected { 1.0 } else { 0.0 },
                // TODO Load the motionScheme tokens from the component tokens file
                animation_spec = MotionSchemeKeyTokens::DefaultEffects.value::<f32>(),
            );
            let size_animation_progress: State<f32> = animate_float_as_state(
                if selected { 1.0 } else { 0.0 },
                // TODO Load the motionScheme tokens from the component tokens file
                animation_spec = MotionSchemeKeyTokens::FastSpatial.value::<f32>(),
            );

            // The entire item is selectable, but only the indicator pill shows the ripple. To achieve
            // this, we re-map the coordinates of the item's InteractionSource into the coordinates of
            // the indicator.
            let density = LocalDensity.current();
            let offset_interaction_source = remember_keyed((interaction_source.clone(), density), || {
                let calculate_delta_offset = move || {
                    let item_width = density.round_to_px(NavigationRailItemWidth);
                    let indicator_width = density.round_to_px(NavigationRailVerticalItemTokens::ActiveIndicatorWidth);
                    Offset::new((item_width - indicator_width) as f32 / 2.0, 0.0)
                };
                MappedInteractionSource::new(&InteractionSource::from(&interaction_source), calculate_delta_offset)
            });

            let indicator_shape: Arc<dyn Shape> = if has_label {
                NavigationRailBaselineItemTokens::ActiveIndicatorShape.value()
            } else {
                ShapeKeyTokens::CornerFull.value()
            };

            // The indicator has a width-expansion animation which interferes with the timing of the
            // ripple, which is why they are separate composables
            let indicator_ripple: Rc<dyn Fn()> = {
                let (indicator_shape, offset_interaction_source) =
                    (indicator_shape.clone(), offset_interaction_source.clone());
                Rc::new(move || {
                    Box(modifier = Modifier
                        .layout_id(IndicatorRippleLayoutIdTag)
                        .clip(ClipShape(indicator_shape.clone()))
                        .indication(offset_interaction_source.source(), Some(named_args!(ripple()))));
                })
            };
            let indicator: Rc<dyn Fn()> = {
                let (indicator_shape, alpha_animation_progress) =
                    (indicator_shape.clone(), alpha_animation_progress.clone());
                Rc::new(move || {
                    let alpha_animation_progress = alpha_animation_progress.clone();
                    Box(modifier = Modifier
                        .layout_id(IndicatorLayoutIdTag)
                        .graphics_layer_block(move |l| l.alpha = alpha_animation_progress.value())
                        .background(colors.indicator_color(), indicator_shape.clone()));
                })
            };

            let (alpha, size) = (alpha_animation_progress.clone(), size_animation_progress.clone());
            NavigationRailItemLayout(
                indicator_ripple,
                indicator,
                styled_icon.clone(),
                styled_label.clone(),
                always_show_label,
                Rc::new(move || alpha.value()),
                Rc::new(move || size.value()),
            );
        },
    );
}

/// The navigation rail's defaults (Compose's `NavigationRailDefaults`).
pub struct NavigationRailDefaults;

impl NavigationRailDefaults {
    /// A navigation rail's background: the scheme's surface (Compose's `ContainerColor`).
    pub fn ContainerColor() -> Color {
        NavigationRailCollapsedTokens::ContainerColor.value()
    }

    /// The insets a navigation rail keeps clear of: the system bars' at the top, the bottom and the start
    /// (Compose's `windowInsets`).
    #[track_caller]
    pub fn window_insets() -> WindowInsets {
        system_bars_for_visual_components().only(WindowInsetsSides::Vertical + WindowInsetsSides::Start)
    }
}

/// The navigation rail items' defaults (Compose's `NavigationRailItemDefaults`), used as a value
/// (`NavigationRailItemDefaults.colors()`).
pub struct NavigationRailItemDefaults;

#[named]
impl NavigationRailItemDefaults {
    /// A navigation rail item's colors (Compose's `NavigationRailItemDefaults.colors()` and `colors(selectedIconColor,
    /// …)`): by default the scheme's — selected, the icon in on-secondary-container, the label in secondary, the
    /// pill in secondary container; unselected, both in on-surface-variant; disabled, the unselected ones at 38% (None
    /// is Kotlin's default for `disabled_icon_color` and `disabled_text_color`: `unselected_icon_color` /
    /// `unselected_text_color` at 38%).
    ///
    /// Kotlin: `colors(selectedIconColor: Color = NavigationRailColorTokens.ItemActiveIcon.value, selectedTextColor: Color = NavigationRailColorTokens.ItemActiveLabelText.value, indicatorColor: Color = NavigationRailColorTokens.ItemActiveIndicator.value, unselectedIconColor: Color = NavigationRailColorTokens.ItemInactiveIcon.value, unselectedTextColor: Color = NavigationRailColorTokens.ItemInactiveLabelText.value, disabledIconColor: Color = unselectedIconColor.copy(alpha = DisabledAlpha), disabledTextColor: Color = unselectedTextColor.copy(alpha = DisabledAlpha)): NavigationRailItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(NavigationRailColorTokens::ItemActiveIcon.value())] selected_icon_color: Color,
        #[default(NavigationRailColorTokens::ItemActiveLabelText.value())] selected_text_color: Color,
        #[default(NavigationRailColorTokens::ItemActiveIndicator.value())] indicator_color: Color,
        #[default(NavigationRailColorTokens::ItemInactiveIcon.value())] unselected_icon_color: Color,
        #[default(NavigationRailColorTokens::ItemInactiveLabelText.value())] unselected_text_color: Color,
        #[default(None)] disabled_icon_color: Option<Color>,
        #[default(None)] disabled_text_color: Option<Color>,
    ) -> NavigationRailItemColors {
        let disabled_icon_color =
            disabled_icon_color.unwrap_or_else(|| named_args!(unselected_icon_color.copy(alpha = DisabledAlpha)));
        let disabled_text_color =
            disabled_text_color.unwrap_or_else(|| named_args!(unselected_text_color.copy(alpha = DisabledAlpha)));
        named_args!(
            NavigationRailItemDefaults::default_navigation_rail_item_colors(&MaterialTheme::color_scheme()).copy(
                selected_icon_color = selected_icon_color,
                selected_text_color = selected_text_color,
                selected_indicator_color = indicator_color,
                unselected_icon_color = unselected_icon_color,
                unselected_text_color = unselected_text_color,
                disabled_icon_color = disabled_icon_color,
                disabled_text_color = disabled_text_color,
            )
        )
    }
}

impl NavigationRailItemDefaults {
    /// The scheme's navigation rail item colors (Compose's internal `ColorScheme.defaultNavigationRailItemColors`;
    /// made afresh, as KompozR keeps no per-scheme caches).
    pub(crate) fn default_navigation_rail_item_colors(scheme: &ColorScheme) -> NavigationRailItemColors {
        NavigationRailItemColors {
            selected_icon_color: scheme.from_token(NavigationRailColorTokens::ItemActiveIcon),
            selected_text_color: scheme.from_token(NavigationRailColorTokens::ItemActiveLabelText),
            selected_indicator_color: scheme.from_token(NavigationRailColorTokens::ItemActiveIndicator),
            unselected_icon_color: scheme.from_token(NavigationRailColorTokens::ItemInactiveIcon),
            unselected_text_color: scheme.from_token(NavigationRailColorTokens::ItemInactiveLabelText),
            disabled_icon_color: named_args!(
                scheme.from_token(NavigationRailColorTokens::ItemInactiveIcon).copy(alpha = DisabledAlpha)
            ),
            disabled_text_color: named_args!(
                scheme.from_token(NavigationRailColorTokens::ItemInactiveLabelText).copy(alpha = DisabledAlpha)
            ),
        }
    }
}

/// A navigation rail item's colors (Compose's `NavigationRailItemColors`): its icon's and label's, selected,
/// unselected and disabled, and its pill's. Equal by its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct NavigationRailItemColors {
    /// The icon's color, selected.
    pub selected_icon_color: Color,
    /// The label's color, selected.
    pub selected_text_color: Color,
    /// The pill's color (shown while selected).
    pub selected_indicator_color: Color,
    /// The icon's color, unselected.
    pub unselected_icon_color: Color,
    /// The label's color, unselected.
    pub unselected_text_color: Color,
    /// The icon's color, disabled.
    pub disabled_icon_color: Color,
    /// The label's color, disabled.
    pub disabled_text_color: Color,
}

impl NavigationRailItemColors {
    /// Colors of one's own (Compose's constructor).
    ///
    /// Kotlin: `NavigationRailItemColors(selectedIconColor: Color, selectedTextColor: Color, selectedIndicatorColor: Color, unselectedIconColor: Color, unselectedTextColor: Color, disabledIconColor: Color, disabledTextColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        selected_icon_color: Color,
        selected_text_color: Color,
        selected_indicator_color: Color,
        unselected_icon_color: Color,
        unselected_text_color: Color,
        disabled_icon_color: Color,
        disabled_text_color: Color,
    ) -> NavigationRailItemColors {
        NavigationRailItemColors {
            selected_icon_color,
            selected_text_color,
            selected_indicator_color,
            unselected_icon_color,
            unselected_text_color,
            disabled_icon_color,
            disabled_text_color,
        }
    }

    /// The icon's color: disabled, else selected or not (Compose's internal `iconColor(selected, enabled)`).
    pub(crate) fn icon_color(&self, selected: bool, enabled: bool) -> Color {
        if !enabled {
            self.disabled_icon_color
        } else if selected {
            self.selected_icon_color
        } else {
            self.unselected_icon_color
        }
    }

    /// The label's color: disabled, else selected or not (Compose's internal `textColor(selected, enabled)`).
    pub(crate) fn text_color(&self, selected: bool, enabled: bool) -> Color {
        if !enabled {
            self.disabled_text_color
        } else if selected {
            self.selected_text_color
        } else {
            self.unselected_text_color
        }
    }

    /// The pill's color (Compose's internal `indicatorColor`).
    pub(crate) fn indicator_color(&self) -> Color {
        self.selected_indicator_color
    }
}

#[named]
impl NavigationRailItemColors {
    /// These colors with those given in place of their own (Compose's `copy`): an unspecified one keeps its own.
    #[allow(clippy::too_many_arguments)]
    pub fn copy(
        &self,
        #[default(self.selected_icon_color)] selected_icon_color: Color,
        #[default(self.selected_text_color)] selected_text_color: Color,
        #[default(self.selected_indicator_color)] selected_indicator_color: Color,
        #[default(self.unselected_icon_color)] unselected_icon_color: Color,
        #[default(self.unselected_text_color)] unselected_text_color: Color,
        #[default(self.disabled_icon_color)] disabled_icon_color: Color,
        #[default(self.disabled_text_color)] disabled_text_color: Color,
    ) -> NavigationRailItemColors {
        NavigationRailItemColors::new(
            selected_icon_color.take_or_else(|| self.selected_icon_color),
            selected_text_color.take_or_else(|| self.selected_text_color),
            selected_indicator_color.take_or_else(|| self.selected_indicator_color),
            unselected_icon_color.take_or_else(|| self.unselected_icon_color),
            unselected_text_color.take_or_else(|| self.unselected_text_color),
            disabled_icon_color.take_or_else(|| self.disabled_icon_color),
            disabled_text_color.take_or_else(|| self.disabled_text_color),
        )
    }
}

/// A navigation rail item's parts laid out (Compose's private `NavigationRailItemLayout`): the indicator's ripple
/// and the indicator (as wide as the icon and 16 dp on each side, the indicator that much of the way in as it is
/// picked), the icon, and the label (if any; faded in with the indicator unless always shown) — see
/// [`place_icon`] and [`place_label_and_icon`]. It gives a badge the bounds it keeps in.
#[kompozable]
fn NavigationRailItemLayout(
    indicator_ripple: Rc<dyn Fn()>,
    indicator: Rc<dyn Fn()>,
    icon: Rc<dyn Fn()>,
    label: Option<Rc<dyn Fn()>>,
    always_show_label: bool,
    alpha_animation_progress: Rc<dyn Fn() -> f32>,
    size_animation_progress: Rc<dyn Fn() -> f32>,
) {
    let has_label = label.is_some();
    Layout(
        modifier = badge_bounds(Modifier),
        content = move || {
            indicator_ripple();
            indicator();

            let icon = icon.clone();
            Box(modifier = Modifier.layout_id(IconLayoutIdTag), content = move || icon());

            if let Some(label) = label.clone() {
                let alpha_animation_progress = alpha_animation_progress.clone();
                Box(
                    modifier = Modifier.layout_id(LabelLayoutIdTag).graphics_layer_block(move |l| {
                        l.alpha = if always_show_label { 1.0 } else { alpha_animation_progress() }
                    }),
                    content = move || label(),
                );
            }
        },
        measure_policy = measure_policy(move |scope, measurables, constraints| {
            // Ensure that the progress is >= 0. It may be negative on bouncy springs, for example.
            let animation_progress = size_animation_progress().max(0.0);
            let loose_constraints = named_args!(constraints.copy(min_width = 0, min_height = 0));
            let icon_placeable = find(measurables, IconLayoutIdTag).measure(loose_constraints);

            let total_indicator_width = icon_placeable.width() + scope.round_to_px(IndicatorHorizontalPadding * 2.0);
            let animated_indicator_width = (total_indicator_width as f32 * animation_progress).round_to_int();
            let indicator_vertical_padding =
                if has_label { IndicatorVerticalPaddingWithLabel } else { IndicatorVerticalPaddingNoLabel };
            let indicator_height = icon_placeable.height() + scope.round_to_px(indicator_vertical_padding * 2.0);

            let indicator_ripple_placeable = find(measurables, IndicatorRippleLayoutIdTag)
                .measure(Constraints::fixed(total_indicator_width, indicator_height));
            let indicator_placeable = measurables
                .iter()
                .find(|m| m.layout_id().is_some_and(|id| id.is(&IndicatorLayoutIdTag)))
                .map(|m| m.measure(Constraints::fixed(animated_indicator_width, indicator_height)));

            let label_placeable = has_label.then(|| find(measurables, LabelLayoutIdTag).measure(loose_constraints));

            match label_placeable {
                None => place_icon(scope, icon_placeable, indicator_ripple_placeable, indicator_placeable, constraints),
                Some(label_placeable) => place_label_and_icon(
                    scope,
                    label_placeable,
                    icon_placeable,
                    indicator_ripple_placeable,
                    indicator_placeable,
                    constraints,
                    always_show_label,
                    animation_progress,
                ),
            }
        }),
    );
}

/// The child of `measurables` laid out under `id` (Compose's `fastFirst { it.layoutId == id }`).
fn find<'a>(measurables: &[&'a dyn Measurable], id: &'static str) -> &'a dyn Measurable {
    measurables
        .iter()
        .find(|m| m.layout_id().is_some_and(|key| key.is(&id)))
        .copied()
        .unwrap_or_else(|| panic!("NavigationRailItemLayout: no child laid out as {id:?}"))
}

/// The icon, the indicator and its ripple centered in the item, 56 dp tall or as the constraints say (Compose's
/// private `placeIcon`).
fn place_icon(
    scope: &MeasureScope,
    icon_placeable: Placeable,
    indicator_ripple_placeable: Placeable,
    indicator_placeable: Option<Placeable>,
    constraints: Constraints,
) -> MeasureResult {
    let width = constraints.constrain_width(
        icon_placeable
            .width()
            .max(indicator_ripple_placeable.width())
            .max(indicator_placeable.as_ref().map_or(0, |p| p.width())),
    );
    let height = constraints.constrain_height(scope.round_to_px(NavigationRailItemHeight));

    let icon_x = (width - icon_placeable.width()) / 2;
    let icon_y = (height - icon_placeable.height()) / 2;

    let ripple_x = (width - indicator_ripple_placeable.width()) / 2;
    let ripple_y = (height - indicator_ripple_placeable.height()) / 2;

    scope.layout(width, height, move |p| {
        if let Some(it) = &indicator_placeable {
            let indicator_x = (width - it.width()) / 2;
            let indicator_y = (height - it.height()) / 2;
            p.place_relative(it, indicator_x, indicator_y);
        }
        p.place_relative(&icon_placeable, icon_x, icon_y);
        p.place_relative(&indicator_ripple_placeable, ripple_x, ripple_y);
    })
}

/// The icon and the label (Compose's private `placeLabelAndIcon`): together in the middle of the item, the label
/// 8 dp under the icon's pill, while selected or with `always_show_label`; unselected without it, the icon alone in
/// the middle (the label not placed); between them as `animation_progress` goes. The indicator and its ripple stay
/// round the icon.
#[allow(clippy::too_many_arguments)]
fn place_label_and_icon(
    scope: &MeasureScope,
    label_placeable: Placeable,
    icon_placeable: Placeable,
    indicator_ripple_placeable: Placeable,
    indicator_placeable: Option<Placeable>,
    constraints: Constraints,
    always_show_label: bool,
    animation_progress: f32,
) -> MeasureResult {
    let content_height = icon_placeable.height() as f32
        + scope.to_px(IndicatorVerticalPaddingWithLabel)
        + scope.to_px(NavigationRailItemVerticalPadding)
        + label_placeable.height() as f32;
    let content_vertical_padding =
        ((constraints.min_height as f32 - content_height) / 2.0).max(scope.to_px(IndicatorVerticalPaddingWithLabel));
    let height = content_height + content_vertical_padding * 2.0;

    // Icon (when selected) should be `contentVerticalPadding` from the top
    let selected_icon_y = content_vertical_padding;
    let unselected_icon_y =
        if always_show_label { selected_icon_y } else { (height - icon_placeable.height() as f32) / 2.0 };

    // How far the icon needs to move between unselected and selected states
    let icon_distance = unselected_icon_y - selected_icon_y;

    // The interpolated fraction of iconDistance that all placeables need to move based on
    // animationProgress, since the icon is higher in the selected state.
    let offset = icon_distance * (1.0 - animation_progress);

    // Label should be fixed padding below icon
    let label_y = selected_icon_y
        + icon_placeable.height() as f32
        + scope.to_px(IndicatorVerticalPaddingWithLabel)
        + scope.to_px(NavigationRailItemVerticalPadding);

    let width = constraints.constrain_width(
        icon_placeable.width().max(label_placeable.width()).max(indicator_placeable.as_ref().map_or(0, |p| p.width())),
    );
    let label_x = (width - label_placeable.width()) / 2;
    let icon_x = (width - icon_placeable.width()) / 2;
    let ripple_x = (width - indicator_ripple_placeable.width()) / 2;
    let ripple_y = selected_icon_y - scope.to_px(IndicatorVerticalPaddingWithLabel);
    let indicator_padding = scope.to_px(IndicatorVerticalPaddingWithLabel);

    scope.layout(width, height.round_to_int(), move |p| {
        if let Some(it) = &indicator_placeable {
            let indicator_x = (width - it.width()) / 2;
            let indicator_y = selected_icon_y - indicator_padding;
            p.place_relative(it, indicator_x, (indicator_y + offset).round_to_int());
        }
        if always_show_label || animation_progress != 0.0 {
            p.place_relative(&label_placeable, label_x, (label_y + offset).round_to_int());
        }
        p.place_relative(&icon_placeable, icon_x, (selected_icon_y + offset).round_to_int());
        p.place_relative(&indicator_ripple_placeable, ripple_x, (ripple_y + offset).round_to_int());
    })
}

const IndicatorRippleLayoutIdTag: &str = "indicatorRipple";

const IndicatorLayoutIdTag: &str = "indicator";

const IconLayoutIdTag: &str = "icon";

const LabelLayoutIdTag: &str = "label";

/// Vertical padding between the contents of the [`NavigationRail`] and its top/bottom, and internally between
/// items.
pub(crate) const NavigationRailVerticalPadding: Dp = Dp(4.0);

/// Padding at the bottom of the [`NavigationRail`]'s header. This padding will only be added when the header is
/// not null.
const NavigationRailHeaderPadding: Dp = Dp(8.0);

/// Width of an individual [`NavigationRailItem`].
pub(crate) const NavigationRailItemWidth: Dp = NavigationRailCollapsedTokens::NarrowContainerWidth;

/// Height of an individual [`NavigationRailItem`].
pub(crate) const NavigationRailItemHeight: Dp = NavigationRailVerticalItemTokens::ActiveIndicatorWidth;

/// Vertical padding between the contents of a [`NavigationRailItem`] and its top/bottom.
pub(crate) const NavigationRailItemVerticalPadding: Dp = Dp(4.0);

const IndicatorHorizontalPadding: Dp =
    Dp((NavigationRailVerticalItemTokens::ActiveIndicatorWidth.0 - NavigationRailBaselineItemTokens::IconSize.0) / 2.0);

const IndicatorVerticalPaddingWithLabel: Dp = Dp((NavigationRailVerticalItemTokens::ActiveIndicatorHeight.0
    - NavigationRailBaselineItemTokens::IconSize.0)
    / 2.0);

const IndicatorVerticalPaddingNoLabel: Dp =
    Dp((NavigationRailVerticalItemTokens::ActiveIndicatorWidth.0 - NavigationRailBaselineItemTokens::IconSize.0) / 2.0);
