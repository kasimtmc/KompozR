//! Material's navigation bar, as Compose's NavigationBar.kt (material3 1.5.0-alpha17): a row of three to five
//! destinations along the bottom of the screen ([`NavigationBar`], 80 dp tall, clear of the system bars at the
//! bottom and the sides), each a [`NavigationBarItem`] taking an equal share of the width: an icon with an optional
//! label under it (shown always or only while selected), a pill behind the icon growing in from its middle as the
//! item is picked (the ripple in the pill, where it was pressed), the colors of [`NavigationBarItemColors`] eased
//! between. How a bar is made can be replaced below a [`LocalNavigationBarOverride`] (Compose's experimental
//! override API).

use crate::badge::badge_bounds;
use crate::color_scheme::{ColorScheme, DisabledAlpha};
use crate::content_color::LocalContentColor;
use crate::internal::mapped_interaction_source::MappedInteractionSource;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::internal::shape_util::ClipShape;
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::material_theme::MaterialTheme;
use crate::ripples::ripple;
use crate::surface::Surface;
use crate::tokens::{ElevationTokens, MotionSchemeKeyTokens, NavigationBarTokens, NavigationBarVerticalItemTokens};
use kompoz_animation::{animate_color_as_state, animate_float_as_state};
use kompoz_foundation::interaction::{InteractionSource, MutableInteractionSource};
use kompoz_foundation::layout::{WindowInsets, WindowInsetsSides};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Constraints, Dp, Offset};
use kompoz_runtime::{
    KompozitionLocal, KompozitionLocalProvider, KotlinProperties, State, kompozable, kompozable_block, named,
    named_args, remember, remember_keyed,
};
use kompoz_ui::layout::{Layout, Measurable, MeasureResult, MeasureScope, Placeable, measure_policy};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::rc::Rc;

/// A navigation bar (Compose's `NavigationBar`): a [`Surface`] of `container_color` (the scheme's surface container
/// by default) at `tonal_elevation`, holding a row as wide as it may be, at least 80 dp tall, padded by
/// `window_insets` (the system bars at the bottom and the sides by default), its items centered across and 8 dp
/// apart — `content`, typically 3 to 5 [`NavigationBarItem`]s (Kotlin's `RowScope`: the Kotlin syntax calls an
/// item's `RowScope` form inside). The row is a selectable group. Content takes `content_color` (None is Kotlin's
/// default, the scheme's `contentColorFor(container_color)`). Made by [`LocalNavigationBarOverride`]'s override
/// ([`DefaultNavigationBarOverride`] unless provided).
///
/// Kotlin: `NavigationBar(modifier: Modifier = Modifier, containerColor: Color = NavigationBarDefaults.containerColor, contentColor: Color = MaterialTheme.colorScheme.contentColorFor(containerColor), tonalElevation: Dp = NavigationBarDefaults.Elevation, windowInsets: WindowInsets = NavigationBarDefaults.windowInsets, content: @Composable RowScope.() -> Unit)`
#[kompozable]
pub fn NavigationBar(
    #[default(Modifier)] modifier: Modifier,
    #[default(NavigationBarDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(NavigationBarDefaults::Elevation)] tonal_elevation: Dp,
    #[default(NavigationBarDefaults::window_insets())] window_insets: WindowInsets,
    content: impl Fn() + 'static,
) {
    let content_color =
        content_color.unwrap_or_else(|| MaterialTheme::color_scheme().content_color_for(container_color));
    let override_ = LocalNavigationBarOverride.current();
    override_.NavigationBar(&NavigationBarOverrideScope {
        modifier,
        container_color,
        content_color,
        tonal_elevation,
        window_insets,
        content: Rc::new(content),
    });
}

/// The navigation bar of an override (Compose's experimental `NavigationBarOverride`): what makes every
/// [`NavigationBar`] below a [`LocalNavigationBarOverride`] providing it, from the scope's arguments.
pub trait NavigationBarOverride {
    /// Composes the bar of `scope`'s arguments (Compose's `NavigationBarOverrideScope.NavigationBar()`).
    fn NavigationBar(&self, scope: &NavigationBarOverrideScope);
}

impl PartialEq for dyn NavigationBarOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default way a navigation bar is made (Compose's experimental `DefaultNavigationBarOverride`): a surface
/// holding the items in a row, as [`NavigationBar`] says.
#[derive(Clone, Copy, Debug)]
pub struct DefaultNavigationBarOverride;

impl NavigationBarOverride for DefaultNavigationBarOverride {
    #[track_caller]
    fn NavigationBar(&self, scope: &NavigationBarOverrideScope) {
        let NavigationBarOverrideScope {
            modifier,
            container_color,
            content_color,
            tonal_elevation,
            window_insets,
            content,
        } = scope.clone();
        kompozable_block!({
            Surface(
                color = container_color,
                content_color = Some(content_color),
                tonal_elevation = tonal_elevation,
                modifier = modifier,
                content = || {
                    let content = content.clone();
                    Row(
                        modifier = named_args!(
                            Modifier
                                .fill_max_width()
                                .window_insets_padding(window_insets.clone())
                                .default_min_size(min_height = NavigationBarHeight)
                        )
                        .selectable_group(),
                        horizontal_arrangement = Arrangement::spaced_by(NavigationBarItemHorizontalPadding),
                        vertical_alignment = Alignment::CenterVertically,
                        content = move || content(),
                    );
                },
            );
        })
    }
}

/// The arguments of a [`NavigationBar`], given to its override (Compose's experimental
/// `NavigationBarOverrideScope`). Made by [`NavigationBar`].
#[derive(Clone)]
pub struct NavigationBarOverrideScope {
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
    tonal_elevation: Dp,
    window_insets: WindowInsets,
    content: Rc<dyn Fn()>,
}

impl NavigationBarOverrideScope {
    /// The bar's modifier (Compose's `modifier`).
    pub fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }

    /// The bar's background (Compose's `containerColor`).
    pub fn container_color(&self) -> Color {
        self.container_color
    }

    /// The color of the bar's content (Compose's `contentColor`).
    pub fn content_color(&self) -> Color {
        self.content_color
    }

    /// The bar's tonal elevation (Compose's `tonalElevation`).
    pub fn tonal_elevation(&self) -> Dp {
        self.tonal_elevation
    }

    /// The insets the bar keeps clear of (Compose's `windowInsets`).
    pub fn window_insets(&self) -> WindowInsets {
        self.window_insets.clone()
    }

    /// The bar's items (Compose's `content`, a `RowScope` lambda).
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the navigation bars below (Compose's experimental `LocalNavigationBarOverride`):
/// [`DefaultNavigationBarOverride`] unless provided.
pub static LocalNavigationBarOverride: KompozitionLocal<Rc<dyn NavigationBarOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultNavigationBarOverride));

/// A navigation bar item (Compose's `RowScope.NavigationBarItem`; Kotlin's receiver scope: the Kotlin syntax calls
/// it for `NavigationBarItem(…)` inside a [`NavigationBar`]): an equal share of the bar's width (its weight 1), at
/// least 80 dp tall, the whole item selectable (the Tab role, `on_click` unless disabled). Its `icon` (in its color
/// by `colors`, eased on the default effects spec) above its `label` (label medium, in its color), both centered;
/// with `always_show_label` false the label shows only while selected, the icon in the middle otherwise and moving
/// up as the item is picked. A pill (the selected indicator color, 56 × 32 dp, fully rounded) behind the icon grows
/// out of its middle and fades in as it is selected; the ripple is in the pill only, where the item was pressed.
/// A label makes the icon's description silent to accessibility while it shows. Its interactions go to
/// `interaction_source` (one of its own where none). It gives a badge the bounds it keeps in.
///
/// Kotlin: `RowScope.NavigationBarItem(selected: Boolean, onClick: () -> Unit, icon: @Composable () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, label: @Composable (() -> Unit)? = null, alwaysShowLabel: Boolean = true, colors: NavigationBarItemColors = NavigationBarItemDefaults.colors(), interactionSource: MutableInteractionSource? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn NavigationBarItem(
    selected: bool,
    on_click: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] label: Option<Rc<dyn Fn()>>,
    #[default(true)] always_show_label: bool,
    #[default(NavigationBarItemDefaults.colors())] colors: NavigationBarItemColors,
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
            let style = NavigationBarTokens::LabelTextFont.value();
            let text_color = animate_color_as_state(
                colors.text_color(selected, enabled),
                animation_spec = color_animation_spec.clone(),
            );
            let label = label.clone();
            ProvideContentColorTextStyle(text_color.value(), style, move || label());
        }) as Rc<dyn Fn()>
    });

    let item_width = remember(|| kompoz_runtime::mutable_int_state_of(0));
    let set_width = item_width.clone();
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
            .default_min_size_with(Dp::Unspecified, NavigationBarHeight)
            .weight_with(1.0, true)
            .on_size_changed(move |size| set_width.set(size.width)),
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
            // (Compose's `remember(interactionSource, calculateDeltaOffset)`: its lambda kept while what it
            // captures — the density, the item's width state — is)
            let density = LocalDensity.current();
            let offset_interaction_source = {
                let item_width = item_width.clone();
                remember_keyed((interaction_source.clone(), density), || {
                    let calculate_delta_offset = move || {
                        let indicator_width =
                            density.round_to_px(NavigationBarVerticalItemTokens::ActiveIndicatorWidth);
                        Offset::new(
                            (item_width.get() - indicator_width) as f32 / 2.0,
                            density.to_px(IndicatorVerticalOffset),
                        )
                    };
                    MappedInteractionSource::new(&InteractionSource::from(&interaction_source), calculate_delta_offset)
                })
            };

            // The indicator has a width-expansion animation which interferes with the timing of the
            // ripple, which is why they are separate composables
            let indicator_ripple: Rc<dyn Fn()> = Rc::new(move || {
                Box(modifier = Modifier
                    .layout_id(IndicatorRippleLayoutIdTag)
                    .clip(ClipShape(NavigationBarTokens::ItemActiveIndicatorShape.value()))
                    .indication(offset_interaction_source.source(), Some(named_args!(ripple()))));
            });
            let indicator: Rc<dyn Fn()> = {
                let alpha_animation_progress = alpha_animation_progress.clone();
                Rc::new(move || {
                    let alpha_animation_progress = alpha_animation_progress.clone();
                    Box(modifier = Modifier
                        .layout_id(IndicatorLayoutIdTag)
                        .graphics_layer_block(move |l| l.alpha = alpha_animation_progress.value())
                        .background(colors.indicator_color(), NavigationBarTokens::ItemActiveIndicatorShape.value()));
                })
            };

            let (alpha, size) = (alpha_animation_progress.clone(), size_animation_progress.clone());
            NavigationBarItemLayout(
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

/// The navigation bar's defaults (Compose's `NavigationBarDefaults`).
pub struct NavigationBarDefaults;

impl NavigationBarDefaults {
    /// A navigation bar's elevation: none (Compose's `Elevation`).
    pub const Elevation: Dp = ElevationTokens::Level0;

    /// A navigation bar's background: the scheme's surface container (Compose's `containerColor`).
    pub fn container_color() -> Color {
        NavigationBarTokens::ContainerColor.value()
    }

    /// The insets a navigation bar keeps clear of (and consumes): the system bars' at the bottom and the sides
    /// (Compose's `windowInsets`).
    #[track_caller]
    pub fn window_insets() -> WindowInsets {
        system_bars_for_visual_components().only(WindowInsetsSides::Horizontal + WindowInsetsSides::Bottom)
    }
}

/// The navigation bar items' defaults (Compose's `NavigationBarItemDefaults`), used as a value
/// (`NavigationBarItemDefaults.colors()`).
pub struct NavigationBarItemDefaults;

#[named]
impl NavigationBarItemDefaults {
    /// A navigation bar item's colors (Compose's `NavigationBarItemDefaults.colors()` and `colors(selectedIconColor,
    /// …)`): the scheme's — selected, the icon in on-secondary-container, the label in secondary, the pill in
    /// secondary container; unselected, both in on-surface-variant; disabled, the unselected ones at 38% — with each
    /// color given other than unspecified over them.
    ///
    /// Kotlin: `colors(selectedIconColor: Color = Color.Unspecified, selectedTextColor: Color = Color.Unspecified, indicatorColor: Color = Color.Unspecified, unselectedIconColor: Color = Color.Unspecified, unselectedTextColor: Color = Color.Unspecified, disabledIconColor: Color = Color.Unspecified, disabledTextColor: Color = Color.Unspecified): NavigationBarItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(Color::Unspecified)] selected_icon_color: Color,
        #[default(Color::Unspecified)] selected_text_color: Color,
        #[default(Color::Unspecified)] indicator_color: Color,
        #[default(Color::Unspecified)] unselected_icon_color: Color,
        #[default(Color::Unspecified)] unselected_text_color: Color,
        #[default(Color::Unspecified)] disabled_icon_color: Color,
        #[default(Color::Unspecified)] disabled_text_color: Color,
    ) -> NavigationBarItemColors {
        named_args!(NavigationBarItemDefaults::default_navigation_bar_item_colors(&MaterialTheme::color_scheme()).copy(
            selected_icon_color = selected_icon_color,
            selected_text_color = selected_text_color,
            selected_indicator_color = indicator_color,
            unselected_icon_color = unselected_icon_color,
            unselected_text_color = unselected_text_color,
            disabled_icon_color = disabled_icon_color,
            disabled_text_color = disabled_text_color,
        ))
    }
}

impl NavigationBarItemDefaults {
    /// The scheme's navigation bar item colors (Compose's internal `ColorScheme.defaultNavigationBarItemColors`;
    /// made afresh, as KompozR keeps no per-scheme caches).
    pub(crate) fn default_navigation_bar_item_colors(scheme: &ColorScheme) -> NavigationBarItemColors {
        NavigationBarItemColors {
            selected_icon_color: scheme.from_token(NavigationBarTokens::ItemActiveIconColor),
            selected_text_color: scheme.from_token(NavigationBarTokens::ItemActiveLabelTextColor),
            selected_indicator_color: scheme.from_token(NavigationBarTokens::ItemActiveIndicatorColor),
            unselected_icon_color: scheme.from_token(NavigationBarTokens::ItemInactiveIconColor),
            unselected_text_color: scheme.from_token(NavigationBarTokens::ItemInactiveLabelTextColor),
            disabled_icon_color: named_args!(
                scheme.from_token(NavigationBarTokens::ItemInactiveIconColor).copy(alpha = DisabledAlpha)
            ),
            disabled_text_color: named_args!(
                scheme.from_token(NavigationBarTokens::ItemInactiveLabelTextColor).copy(alpha = DisabledAlpha)
            ),
        }
    }
}

/// A navigation bar item's colors (Compose's `NavigationBarItemColors`): its icon's and label's, selected,
/// unselected and disabled, and its pill's. Equal by its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct NavigationBarItemColors {
    /// The color to use for the icon when the item is selected.
    pub selected_icon_color: Color,
    /// The color to use for the text label when the item is selected.
    pub selected_text_color: Color,
    /// The color to use for the indicator when the item is selected.
    pub selected_indicator_color: Color,
    /// The color to use for the icon when the item is unselected.
    pub unselected_icon_color: Color,
    /// The color to use for the text label when the item is unselected.
    pub unselected_text_color: Color,
    /// The color to use for the icon when the item is disabled.
    pub disabled_icon_color: Color,
    /// The color to use for the text label when the item is disabled.
    pub disabled_text_color: Color,
}

impl NavigationBarItemColors {
    /// Colors of one's own (Compose's constructor).
    ///
    /// Kotlin: `NavigationBarItemColors(selectedIconColor: Color, selectedTextColor: Color, selectedIndicatorColor: Color, unselectedIconColor: Color, unselectedTextColor: Color, disabledIconColor: Color, disabledTextColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        selected_icon_color: Color,
        selected_text_color: Color,
        selected_indicator_color: Color,
        unselected_icon_color: Color,
        unselected_text_color: Color,
        disabled_icon_color: Color,
        disabled_text_color: Color,
    ) -> NavigationBarItemColors {
        NavigationBarItemColors {
            selected_icon_color,
            selected_text_color,
            selected_indicator_color,
            unselected_icon_color,
            unselected_text_color,
            disabled_icon_color,
            disabled_text_color,
        }
    }

    /// The icon's color, whether the item is `selected` and `enabled` (Compose's internal `iconColor`).
    pub(crate) fn icon_color(&self, selected: bool, enabled: bool) -> Color {
        if !enabled {
            self.disabled_icon_color
        } else if selected {
            self.selected_icon_color
        } else {
            self.unselected_icon_color
        }
    }

    /// The label's color, whether the item is `selected` and `enabled` (Compose's internal `textColor`).
    pub(crate) fn text_color(&self, selected: bool, enabled: bool) -> Color {
        if !enabled {
            self.disabled_text_color
        } else if selected {
            self.selected_text_color
        } else {
            self.unselected_text_color
        }
    }

    /// The pill's color (Compose's internal `indicatorColor`): the selected indicator color.
    pub(crate) fn indicator_color(&self) -> Color {
        self.selected_indicator_color
    }
}

#[named]
impl NavigationBarItemColors {
    /// The same with each color given one other than unspecified (Compose's `copy`: `Color.Unspecified` keeps
    /// this one's).
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
    ) -> NavigationBarItemColors {
        NavigationBarItemColors::new(
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

/// An item's indicator ripple, indicator, icon and label laid out (Compose's private `NavigationBarItemLayout`):
/// the indicator and ripple `IndicatorHorizontalPadding`/`IndicatorVerticalPadding` around the icon, the indicator
/// as wide as `size_animation_progress()` of that (from its middle); a label faded by `alpha_animation_progress()`
/// unless always shown. It gives a badge the bounds it keeps in.
#[kompozable]
fn NavigationBarItemLayout(
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
            let loose_constraints = Constraints::new(0, constraints.max_width, 0, constraints.max_height);
            let icon_placeable = first_by_id(measurables, IconLayoutIdTag).measure(loose_constraints);

            let total_indicator_width = icon_placeable.width() + scope.round_to_px(IndicatorHorizontalPadding * 2.0);
            let animated_indicator_width = round_to_int(total_indicator_width as f32 * animation_progress);
            let indicator_height = icon_placeable.height() + scope.round_to_px(IndicatorVerticalPadding * 2.0);
            let indicator_ripple_placeable = first_by_id(measurables, IndicatorRippleLayoutIdTag)
                .measure(Constraints::fixed(total_indicator_width, indicator_height));
            let indicator_placeable = find_by_id(measurables, IndicatorLayoutIdTag)
                .map(|m| m.measure(Constraints::fixed(animated_indicator_width, indicator_height)));

            let label_placeable =
                has_label.then(|| first_by_id(measurables, LabelLayoutIdTag).measure(loose_constraints));

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

/// The child of `measurables` of the layout id `id` (Compose's `fastFirstOrNull { it.layoutId == id }`).
fn find_by_id<'a>(measurables: &[&'a dyn Measurable], id: &'static str) -> Option<&'a dyn Measurable> {
    measurables.iter().copied().find(|m| m.layout_id().is_some_and(|i| i.is(&id)))
}

/// The child of `measurables` of the layout id `id`, which the layout always has (Compose's `fastFirst`).
fn first_by_id<'a>(measurables: &[&'a dyn Measurable], id: &'static str) -> &'a dyn Measurable {
    find_by_id(measurables, id).unwrap_or_else(|| panic!("a navigation bar item's {id}"))
}

/// A lone icon in the middle of the item, 80 dp tall (or the constraints'), as wide as the constraints let it be
/// (its icon and 44 dp each side where unbounded) (Compose's private `placeIcon`).
fn place_icon(
    scope: &MeasureScope,
    icon_placeable: Placeable,
    indicator_ripple_placeable: Placeable,
    indicator_placeable: Option<Placeable>,
    constraints: Constraints,
) -> MeasureResult {
    let width = if constraints.max_width == Constraints::Infinity {
        icon_placeable.width() + scope.round_to_px(NavigationBarItemToIconMinimumPadding) * 2
    } else {
        constraints.max_width
    };
    let height = constraints.constrain_height(scope.round_to_px(NavigationBarHeight));

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

/// An icon above its label, centered in the item's least height (at least the indicator's vertical padding above
/// and below); a label shown only when selected leaves the icon in the middle until the item is, the icon moving
/// up `animation_progress` of the way (Compose's private `placeLabelAndIcon`).
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
        + scope.to_px(IndicatorVerticalPadding)
        + scope.to_px(NavigationBarIndicatorToLabelPadding)
        + label_placeable.height() as f32;
    let content_vertical_padding =
        ((constraints.min_height as f32 - content_height) / 2.0).max(scope.to_px(IndicatorVerticalPadding));
    let height = content_height + content_vertical_padding * 2.0;

    // Icon (when selected) should be `contentVerticalPadding` from top
    let selected_icon_y = content_vertical_padding;
    let unselected_icon_y =
        if always_show_label { selected_icon_y } else { (height - icon_placeable.height() as f32) / 2.0 };

    // How far the icon needs to move between unselected and selected states.
    let icon_distance = unselected_icon_y - selected_icon_y;

    // The interpolated fraction of iconDistance that all placeables need to move based on
    // animationProgress.
    let offset = icon_distance * (1.0 - animation_progress);

    // Label should be fixed padding below icon
    let label_y = selected_icon_y
        + icon_placeable.height() as f32
        + scope.to_px(IndicatorVerticalPadding)
        + scope.to_px(NavigationBarIndicatorToLabelPadding);

    let container_width = if constraints.max_width == Constraints::Infinity {
        icon_placeable.width() + scope.round_to_px(NavigationBarItemToIconMinimumPadding) * 2
    } else {
        constraints.max_width
    };

    let label_x = (container_width - label_placeable.width()) / 2;
    let icon_x = (container_width - icon_placeable.width()) / 2;

    let ripple_x = (container_width - indicator_ripple_placeable.width()) / 2;
    let ripple_y = selected_icon_y - scope.to_px(IndicatorVerticalPadding);
    let indicator_vertical_padding = scope.round_to_px(IndicatorVerticalPadding);

    scope.layout(container_width, round_to_int(height), move |p| {
        if let Some(it) = &indicator_placeable {
            let indicator_x = (container_width - it.width()) / 2;
            // (Kotlin's Float less an Int: a Float)
            let indicator_y = selected_icon_y - indicator_vertical_padding as f32;
            p.place_relative(it, indicator_x, round_to_int(indicator_y + offset));
        }
        if always_show_label || animation_progress != 0.0 {
            p.place_relative(&label_placeable, label_x, round_to_int(label_y + offset));
        }
        p.place_relative(&icon_placeable, icon_x, round_to_int(selected_icon_y + offset));
        p.place_relative(&indicator_ripple_placeable, ripple_x, round_to_int(ripple_y + offset));
    })
}

/// Kotlin's `Float.roundToInt` (Math.round: a half up).
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

const IndicatorRippleLayoutIdTag: &str = "indicatorRipple";

const IndicatorLayoutIdTag: &str = "indicator";

const IconLayoutIdTag: &str = "icon";

const LabelLayoutIdTag: &str = "label";

const NavigationBarHeight: Dp = NavigationBarTokens::TallContainerHeight;

/*@VisibleForTesting*/
pub(crate) const NavigationBarItemHorizontalPadding: Dp = Dp(8.0);

/*@VisibleForTesting*/
pub(crate) const NavigationBarIndicatorToLabelPadding: Dp = Dp(4.0);

const IndicatorHorizontalPadding: Dp =
    Dp((NavigationBarVerticalItemTokens::ActiveIndicatorWidth.0 - NavigationBarVerticalItemTokens::IconSize.0) / 2.0);

/*@VisibleForTesting*/
pub(crate) const IndicatorVerticalPadding: Dp =
    Dp((NavigationBarVerticalItemTokens::ActiveIndicatorHeight.0 - NavigationBarVerticalItemTokens::IconSize.0) / 2.0);

const IndicatorVerticalOffset: Dp = Dp(12.0);

/*@VisibleForTesting*/
pub(crate) const NavigationBarItemToIconMinimumPadding: Dp = Dp(44.0);
