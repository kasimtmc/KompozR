//! Material's short navigation bar, as Compose's ShortNavigationBar.kt (material3 1.5.0-alpha17, Expressive): a
//! 64 dp row of destinations along the bottom ([`ShortNavigationBar`]), sharing its width equally or centered
//! together by its [`ShortNavigationBarArrangement`], each a [`ShortNavigationBarItem`] — the 1.5 navigation item
//! (crate::navigation_item) with its icon on top of its label (phones in portrait) or at its start (medium
//! widths), a pill behind it growing in as it is picked. How a bar is made can be replaced below a
//! [`LocalShortNavigationBarOverride`] (Compose's experimental override API).

use crate::color_scheme::{ColorScheme, DisabledAlpha, content_color_for};
use crate::internal::system_bars_default_insets::system_bars_for_visual_components;
use crate::material_theme::MaterialTheme;
use crate::navigation_item::{NavigationItem, NavigationItemColors, NavigationItemIconPosition};
use crate::surface::Surface;
use crate::tokens::{NavigationBarHorizontalItemTokens, NavigationBarTokens, NavigationBarVerticalItemTokens};
use kompoz_foundation::interaction::MutableInteractionSource;
use kompoz_foundation::layout::{WindowInsets, WindowInsetsSides};
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Constraints, Dp};
use kompoz_runtime::{KompozitionLocal, kompozable, kompozable_block, named, named_args, remember};
use kompoz_ui::Modifier;
use kompoz_ui::layout::{Layout, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Placeable};
use std::fmt;
use std::rc::Rc;

/// A short navigation bar (Compose's Expressive `ShortNavigationBar`): a [`Surface`] of `container_color` (the
/// scheme's surface container by default), its content in `content_color` (None is Kotlin's default,
/// [`ShortNavigationBarDefaults::content_color`]), holding a layout at least 64 dp tall padded by `window_insets`
/// (the system bars at the bottom and the sides by default), a selectable group, its `content` (typically
/// [`ShortNavigationBarItem`]s) as `arrangement` says: sharing the width equally
/// ([`ShortNavigationBarArrangement::EqualWeight`], the default) or centered together
/// ([`ShortNavigationBarArrangement::Centered`]); all the items as tall as the tallest. Made by
/// [`LocalShortNavigationBarOverride`]'s override ([`DefaultShortNavigationBarOverride`] unless provided).
///
/// Kotlin: `ShortNavigationBar(modifier: Modifier = Modifier, containerColor: Color = ShortNavigationBarDefaults.containerColor, contentColor: Color = ShortNavigationBarDefaults.contentColor, windowInsets: WindowInsets = ShortNavigationBarDefaults.windowInsets, arrangement: ShortNavigationBarArrangement = ShortNavigationBarDefaults.arrangement, content: @Composable () -> Unit)`
#[kompozable]
pub fn ShortNavigationBar(
    #[default(Modifier)] modifier: Modifier,
    #[default(ShortNavigationBarDefaults::container_color())] container_color: Color,
    #[default(None)] content_color: Option<Color>,
    #[default(ShortNavigationBarDefaults::window_insets())] window_insets: WindowInsets,
    #[default(ShortNavigationBarDefaults::arrangement())] arrangement: ShortNavigationBarArrangement,
    content: impl Fn() + 'static,
) {
    let content_color = content_color.unwrap_or_else(ShortNavigationBarDefaults::content_color);
    let override_ = LocalShortNavigationBarOverride.current();
    override_.ShortNavigationBar(&ShortNavigationBarOverrideScope {
        modifier,
        container_color,
        content_color,
        window_insets,
        arrangement,
        content: Rc::new(content),
    });
}

/// The short navigation bar of an override (Compose's experimental `ShortNavigationBarOverride`): what makes
/// every [`ShortNavigationBar`] below a [`LocalShortNavigationBarOverride`] providing it, from the scope's
/// arguments.
pub trait ShortNavigationBarOverride {
    /// Composes the bar of `scope`'s arguments (Compose's `ShortNavigationBarOverrideScope.ShortNavigationBar()`).
    fn ShortNavigationBar(&self, scope: &ShortNavigationBarOverrideScope);
}

impl PartialEq for dyn ShortNavigationBarOverride {
    /// The same override (Kotlin's `==` of objects: their identity).
    fn eq(&self, other: &Self) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

/// The default way a short navigation bar is made (Compose's experimental `DefaultShortNavigationBarOverride`): a
/// surface holding the items laid out by the arrangement, as [`ShortNavigationBar`] says.
#[derive(Clone, Copy, Debug)]
pub struct DefaultShortNavigationBarOverride;

impl ShortNavigationBarOverride for DefaultShortNavigationBarOverride {
    #[track_caller]
    fn ShortNavigationBar(&self, scope: &ShortNavigationBarOverrideScope) {
        let ShortNavigationBarOverrideScope {
            modifier,
            container_color,
            content_color,
            window_insets,
            arrangement,
            content,
        } = scope.clone();
        kompozable_block!({
            Surface(
                color = container_color,
                content_color = Some(content_color),
                modifier = modifier,
                content = || {
                    let content = content.clone();
                    let modifier = named_args!(
                        Modifier
                            .window_insets_padding(window_insets.clone())
                            .default_min_size(min_height = NavigationBarTokens::ContainerHeight)
                    )
                    .selectable_group();
                    match arrangement {
                        ShortNavigationBarArrangement::EqualWeight => {
                            Layout(
                                modifier = modifier,
                                content = move || content(),
                                measure_policy = EqualWeightContentMeasurePolicy,
                            );
                        }
                        ShortNavigationBarArrangement::Centered => {
                            Layout(
                                modifier = modifier,
                                content = move || content(),
                                measure_policy = CenteredContentMeasurePolicy,
                            );
                        }
                        _ => panic!("Invalid ItemsArrangement value."),
                    }
                },
            );
        })
    }
}

/// How a short navigation bar lays out its items (Compose's value class `ShortNavigationBarArrangement`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortNavigationBarArrangement(i32);

impl ShortNavigationBarArrangement {
    /// The items are equally distributed on the Short Navigation Bar.
    ///
    /// This configuration is recommended for small width screens.
    pub const EqualWeight: ShortNavigationBarArrangement = ShortNavigationBarArrangement(0);

    /// The items are centered on the Short Navigation Bar.
    ///
    /// This configuration is recommended for medium width screens.
    pub const Centered: ShortNavigationBarArrangement = ShortNavigationBarArrangement(1);
}

/// Compose's toString: the arrangement's name.
impl fmt::Display for ShortNavigationBarArrangement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            ShortNavigationBarArrangement::EqualWeight => "EqualWeight",
            ShortNavigationBarArrangement::Centered => "Centered",
            _ => "Unknown",
        })
    }
}

impl fmt::Debug for ShortNavigationBarArrangement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// A short navigation bar item (Compose's Expressive `ShortNavigationBarItem`): the whole item selectable (the
/// Tab role, `on_click` unless disabled), at least the minimum interactive size each way; its `icon` on top of its
/// `label` (label medium; a label is recommended) or at its start by `icon_position`, in their colors by `colors`.
/// A fully rounded pill (the selected indicator color) behind the icon — 56 × 32 dp on top, 16 dp either side of
/// both and 40 dp tall at the start — grows out of its middle and fades in as it is selected; the ripple is in the
/// pill only, where the item was pressed. Its interactions go to `interaction_source` (one of its own where none).
/// It gives a badge the bounds it keeps in.
///
/// Kotlin: `ShortNavigationBarItem(selected: Boolean, onClick: () -> Unit, icon: @Composable () -> Unit, label: @Composable (() -> Unit)?, modifier: Modifier = Modifier, enabled: Boolean = true, iconPosition: NavigationItemIconPosition = NavigationItemIconPosition.Top, colors: NavigationItemColors = ShortNavigationBarItemDefaults.colors(), interactionSource: MutableInteractionSource? = null)`
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub fn ShortNavigationBarItem(
    selected: bool,
    on_click: impl Fn() + 'static,
    icon: impl Fn() + 'static,
    label: Option<Rc<dyn Fn()>>,
    #[default(Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(NavigationItemIconPosition::Top)] icon_position: NavigationItemIconPosition,
    #[default(ShortNavigationBarItemDefaults.colors())] colors: NavigationItemColors,
    #[default(None)] interaction_source: Option<MutableInteractionSource>,
) {
    let interaction_source = interaction_source.unwrap_or_else(|| remember(MutableInteractionSource::new));

    let is_icon_position_top = icon_position == NavigationItemIconPosition::Top;
    let indicator_horizontal_padding =
        if is_icon_position_top { TopIconIndicatorHorizontalPadding } else { StartIconIndicatorHorizontalPadding };
    let indicator_vertical_padding =
        if is_icon_position_top { TopIconIndicatorVerticalPadding } else { StartIconIndicatorVerticalPadding };

    NavigationItem(
        selected,
        Rc::new(on_click),
        Rc::new(icon),
        NavigationBarTokens::LabelTextFont.value(),
        NavigationBarTokens::ItemActiveIndicatorShape.value(),
        NavigationBarVerticalItemTokens::ActiveIndicatorWidth,
        indicator_horizontal_padding,
        indicator_vertical_padding,
        TopIconIndicatorToLabelPadding,
        StartIconToLabelPadding,
        TopIconItemVerticalPadding,
        colors,
        modifier,
        enabled,
        label,
        icon_position,
        interaction_source,
    );
}

/// The short navigation bar's defaults (Compose's `ShortNavigationBarDefaults`).
pub struct ShortNavigationBarDefaults;

impl ShortNavigationBarDefaults {
    /// A short navigation bar's background: the scheme's surface container (Compose's `containerColor`).
    pub fn container_color() -> Color {
        NavigationBarTokens::ContainerColor.value()
    }

    /// The color of a short navigation bar's content: the content color for its default background (Compose's
    /// `contentColor`).
    pub fn content_color() -> Color {
        content_color_for(ShortNavigationBarDefaults::container_color())
    }

    /// How a short navigation bar lays out its items: sharing its width equally (Compose's `arrangement`).
    pub fn arrangement() -> ShortNavigationBarArrangement {
        ShortNavigationBarArrangement::EqualWeight
    }

    /// The insets a short navigation bar keeps clear of (and consumes): the system bars' at the bottom and the
    /// sides (Compose's `windowInsets`).
    #[track_caller]
    pub fn window_insets() -> WindowInsets {
        system_bars_for_visual_components().only(WindowInsetsSides::Horizontal + WindowInsetsSides::Bottom)
    }
}

/// The short navigation bar items' defaults (Compose's `ShortNavigationBarItemDefaults`), used as a value
/// (`ShortNavigationBarItemDefaults.colors()`).
pub struct ShortNavigationBarItemDefaults;

#[named]
impl ShortNavigationBarItemDefaults {
    /// A short navigation bar item's colors (Compose's `ShortNavigationBarItemDefaults.colors()` and
    /// `colors(selectedIconColor, …)`): by default the scheme's — selected, the icon in on-secondary-container, the
    /// label in secondary, the pill in secondary container; unselected, both in on-surface-variant; disabled, the
    /// unselected ones at 38% (None is Kotlin's default for `disabled_icon_color` and `disabled_text_color`:
    /// `unselected_icon_color` / `unselected_text_color` at 38%).
    ///
    /// Kotlin: `colors(selectedIconColor: Color = NavigationBarTokens.ItemActiveIconColor.value, selectedTextColor: Color = NavigationBarTokens.ItemActiveLabelTextColor.value, selectedIndicatorColor: Color = NavigationBarTokens.ItemActiveIndicatorColor.value, unselectedIconColor: Color = NavigationBarTokens.ItemInactiveIconColor.value, unselectedTextColor: Color = NavigationBarTokens.ItemInactiveLabelTextColor.value, disabledIconColor: Color = unselectedIconColor.copy(alpha = DisabledAlpha), disabledTextColor: Color = unselectedTextColor.copy(alpha = DisabledAlpha)): NavigationItemColors`
    #[allow(clippy::too_many_arguments)]
    pub fn colors(
        &self,
        #[default(NavigationBarTokens::ItemActiveIconColor.value())] selected_icon_color: Color,
        #[default(NavigationBarTokens::ItemActiveLabelTextColor.value())] selected_text_color: Color,
        #[default(NavigationBarTokens::ItemActiveIndicatorColor.value())] selected_indicator_color: Color,
        #[default(NavigationBarTokens::ItemInactiveIconColor.value())] unselected_icon_color: Color,
        #[default(NavigationBarTokens::ItemInactiveLabelTextColor.value())] unselected_text_color: Color,
        #[default(None)] disabled_icon_color: Option<Color>,
        #[default(None)] disabled_text_color: Option<Color>,
    ) -> NavigationItemColors {
        let disabled_icon_color =
            disabled_icon_color.unwrap_or_else(|| named_args!(unselected_icon_color.copy(alpha = DisabledAlpha)));
        let disabled_text_color =
            disabled_text_color.unwrap_or_else(|| named_args!(unselected_text_color.copy(alpha = DisabledAlpha)));
        named_args!(
            ShortNavigationBarItemDefaults::default_short_navigation_bar_item_colors(&MaterialTheme::color_scheme())
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

impl ShortNavigationBarItemDefaults {
    /// The scheme's short navigation bar item colors (Compose's internal
    /// `ColorScheme.defaultShortNavigationBarItemColors`; made afresh, as KompozR keeps no per-scheme caches).
    pub(crate) fn default_short_navigation_bar_item_colors(scheme: &ColorScheme) -> NavigationItemColors {
        NavigationItemColors {
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

/// The arguments of a [`ShortNavigationBar`], given to its override (Compose's experimental
/// `ShortNavigationBarOverrideScope`). Made by [`ShortNavigationBar`].
#[derive(Clone)]
pub struct ShortNavigationBarOverrideScope {
    modifier: Modifier,
    container_color: Color,
    content_color: Color,
    window_insets: WindowInsets,
    arrangement: ShortNavigationBarArrangement,
    content: Rc<dyn Fn()>,
}

impl ShortNavigationBarOverrideScope {
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

    /// The insets the bar keeps clear of (Compose's `windowInsets`).
    pub fn window_insets(&self) -> WindowInsets {
        self.window_insets.clone()
    }

    /// How the bar lays out its items (Compose's `arrangement`).
    pub fn arrangement(&self) -> ShortNavigationBarArrangement {
        self.arrangement
    }

    /// The bar's items (Compose's `content`).
    pub fn content(&self) -> Rc<dyn Fn()> {
        self.content.clone()
    }
}

/// The override that makes the short navigation bars below (Compose's experimental
/// `LocalShortNavigationBarOverride`): [`DefaultShortNavigationBarOverride`] unless provided.
pub static LocalShortNavigationBarOverride: KompozitionLocal<Rc<dyn ShortNavigationBarOverride>> =
    KompozitionLocal::new(|| Rc::new(DefaultShortNavigationBarOverride));

/// The items sharing the bar's width equally, all as tall as the tallest (by their maximum intrinsic heights at
/// that width; Compose's private `EqualWeightContentMeasurePolicy`). Unbounded, each as wide as it is.
struct EqualWeightContentMeasurePolicy;

impl MeasurePolicy for EqualWeightContentMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let width = constraints.max_width;
        let mut item_height = constraints.min_height;
        let items_count = measurables.len() as i32;
        // If there are no items, bar will be empty.
        if items_count < 1 {
            return scope.layout(width, item_height, |_| {});
        }

        let items_placeables: Vec<Placeable> = if !constraints.has_bounded_width() {
            // If width constraint is not bounded, let item containers widths be as big as they are.
            // This may lead to a different items arrangement than the expected.
            measurables
                .iter()
                .map(|it| it.measure(constraints.constrain(Constraints::fixed_height(item_height))))
                .collect()
        } else {
            let item_width = width / items_count;
            for it in measurables {
                let measurable_height = it.max_intrinsic_height(item_width);
                if item_height < measurable_height {
                    item_height = measurable_height.min(constraints.max_height);
                }
            }

            // Make sure the item containers have the same width and height.
            measurables
                .iter()
                .map(|it| it.measure(constraints.constrain(Constraints::fixed(item_width, item_height))))
                .collect()
        };

        scope.layout(width, item_height, move |p| {
            let mut x = 0;
            let y = 0;
            for item in &items_placeables {
                p.place_relative(item, x, y);
                x += item.width();
            }
        })
    }
}

/// The items centered together in the bar (Compose's private `CenteredContentMeasurePolicy`): a padding at each
/// side by how many there are ([`calculate_centered_content_horizontal_padding`]), each item at least an equal
/// share of what's between and as wide as it needs (up to an equal share of the whole width), the padding taking
/// what a wider item needs so that they stay centered; all as tall as the tallest. Unbounded, each as wide as it
/// is.
struct CenteredContentMeasurePolicy;

impl MeasurePolicy for CenteredContentMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let width = constraints.max_width;
        let mut item_height = constraints.min_height;
        let items_count = measurables.len() as i32;
        // If there are no items, bar will be empty.
        if items_count < 1 {
            return scope.layout(width, item_height, |_| {});
        }

        let mut bar_horizontal_padding = 0;
        let items_placeables: Vec<Placeable> = if !constraints.has_bounded_width() {
            // If width constraint is not bounded, let item containers widths be as big as they are.
            // This may lead to a different items arrangement than the expected.
            measurables
                .iter()
                .map(|it| it.measure(constraints.constrain(Constraints::fixed_height(item_height))))
                .collect()
        } else {
            let item_max_width = width / items_count;
            bar_horizontal_padding = calculate_centered_content_horizontal_padding(items_count, width);
            let item_min_width = (width - (bar_horizontal_padding * 2)) / items_count;

            // Make sure the item containers will have the same height.
            for it in measurables {
                let measurable_height = it.max_intrinsic_height(item_min_width);
                if item_height < measurable_height {
                    item_height = measurable_height.min(constraints.max_height);
                }
            }
            measurables
                .iter()
                .map(|it| {
                    let mut current_item_width = item_min_width;
                    let measurable_width = it.max_intrinsic_width(constraints.min_height);
                    if current_item_width < measurable_width {
                        // Let an item container be bigger in width if needed, but limit it to
                        // itemMaxWidth.
                        current_item_width = measurable_width.min(item_max_width);
                        // Update horizontal padding so that items remain centered.
                        bar_horizontal_padding -= (current_item_width - item_min_width) / 2;
                    }

                    it.measure(constraints.constrain(Constraints::fixed(current_item_width, item_height)))
                })
                .collect()
        };

        scope.layout(width, item_height, move |p| {
            let mut x = bar_horizontal_padding;
            let y = 0;
            for item in &items_placeables {
                p.place_relative(item, x, y);
                x += item.width();
            }
        })
    }
}

/// The padding at each side of a bar of centered items, by how many there are (Compose's private
/// `calculateCenteredContentHorizontalPadding`): `(100 - 10 × (items + 3)) / 2` percent of the bar's width, none
/// for more than 6.
fn calculate_centered_content_horizontal_padding(items_count: i32, bar_width: i32) -> i32 {
    if items_count > 6 {
        return 0;
    }
    // Formula to calculate the padding percentage based on the number of items and bar width.
    let padding_percentage = ((100 - 10 * (items_count + 3)) as f32 / 2.0) / 100.0;
    round_to_int(padding_percentage * bar_width as f32)
}

/// Kotlin's `Float.roundToInt` (Math.round: a half up).
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/*@VisibleForTesting*/
pub(crate) const TopIconItemVerticalPadding: Dp = NavigationBarVerticalItemTokens::ContainerBetweenSpace;
/*@VisibleForTesting*/
pub(crate) const TopIconIndicatorVerticalPadding: Dp =
    Dp((NavigationBarVerticalItemTokens::ActiveIndicatorHeight.0 - NavigationBarVerticalItemTokens::IconSize.0) / 2.0);
/*@VisibleForTesting*/
pub(crate) const TopIconIndicatorHorizontalPadding: Dp =
    Dp((NavigationBarVerticalItemTokens::ActiveIndicatorWidth.0 - NavigationBarVerticalItemTokens::IconSize.0) / 2.0);
/*@VisibleForTesting*/
pub(crate) const StartIconIndicatorVerticalPadding: Dp =
    Dp((NavigationBarHorizontalItemTokens::ActiveIndicatorHeight.0 - NavigationBarHorizontalItemTokens::IconSize.0)
        / 2.0);
/*@VisibleForTesting*/
pub(crate) const TopIconIndicatorToLabelPadding: Dp = Dp(4.0);
/*@VisibleForTesting*/
pub(crate) const StartIconIndicatorHorizontalPadding: Dp =
    NavigationBarHorizontalItemTokens::ActiveIndicatorLeadingSpace;
/*@VisibleForTesting*/
pub(crate) const StartIconToLabelPadding: Dp = NavigationBarTokens::ItemActiveIndicatorIconLabelSpace;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_items_are_padded_by_how_many_there_are() {
        // 3 items in 360: (100 - 60) / 2 = 20% each side; 7 or more: none.
        assert_eq!(calculate_centered_content_horizontal_padding(3, 360), 72);
        assert_eq!(calculate_centered_content_horizontal_padding(6, 1000), 50);
        assert_eq!(calculate_centered_content_horizontal_padding(7, 1000), 0);
        assert_eq!(
            format!("{} {:?}", ShortNavigationBarArrangement::EqualWeight, ShortNavigationBarArrangement::Centered),
            "EqualWeight Centered"
        );
    }
}
