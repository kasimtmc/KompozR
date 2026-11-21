//! The navigation item the 1.5 navigation components share, as Compose's NavigationItem.kt: where an item's icon
//! is ([`NavigationItemIconPosition`]: on top of its label or at its start), its colors ([`NavigationItemColors`]),
//! and the internal items — [`NavigationItem`] (a [`ShortNavigationBarItem`](crate::ShortNavigationBarItem)'s) and
//! [`AnimatedNavigationItem`] (a wide navigation rail item's, moving its icon between the two places) — with their
//! layouts: an indicator pill behind the icon (or the icon and its label) growing out of its middle as the item is
//! selected, a ripple of its own over the pill (the item's presses moved into the pill's place), the label in
//! place. [`DynamicPaddingValues`] is the indicator's padding between a collapsed and an expanded one.

use crate::badge::badge_bounds;
use crate::content_color::LocalContentColor;
use crate::interactive_component_size::LocalMinimumInteractiveComponentSize;
use crate::internal::mapped_interaction_source::MappedInteractionSource;
use crate::internal::provide_content_color_text_style::ProvideContentColorTextStyle;
use crate::internal::shape_util::ClipShape;
use crate::ripples::ripple;
use crate::tokens::MotionSchemeKeyTokens;
use kompoz_animation::{AnimationSpec, animate_float_as_state};
use kompoz_foundation::interaction::{InteractionSource, MutableInteractionSource};
use kompoz_foundation::layout::CustomPaddingValues;
use kompoz_foundation::prelude::*;
use kompoz_graphics::{Color, Constraints, Dp, LayoutDirection, Offset, Shape, lerp_dp};
use kompoz_runtime::{
    KompozitionLocalProvider, KotlinProperties, State, derived_state_of, kompozable, mutable_int_state_of, named,
    remember, remember_keyed,
};
use kompoz_text::TextStyle;
use kompoz_ui::layout::{
    CompositingStrategy, IntrinsicMeasurable, IntrinsicMeasureScope, Layout, Measurable, MeasurePolicy, MeasureResult,
    MeasureScope, Placeable,
};
use kompoz_ui::platform::LocalDensity;
use kompoz_ui::semantics::Role;
use kompoz_ui::{Alignment, Modifier};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

// ---- NavigationItemIconPosition --------------------------------------------------------------------------------

/// Where a navigation item's icon is (Compose's value class `NavigationItemIconPosition`): on top of its label or
/// at its start.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NavigationItemIconPosition(i32);

impl NavigationItemIconPosition {
    /// The icon is positioned on top of the label.
    pub const Top: NavigationItemIconPosition = NavigationItemIconPosition(0);
    /// The icon is positioned at the start of the label.
    pub const Start: NavigationItemIconPosition = NavigationItemIconPosition(1);
}

/// Compose's toString: the position's name.
impl fmt::Display for NavigationItemIconPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match *self {
            NavigationItemIconPosition::Top => "Top",
            NavigationItemIconPosition::Start => "Start",
            _ => "Unknown",
        })
    }
}

impl fmt::Debug for NavigationItemIconPosition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

// ---- NavigationItemColors --------------------------------------------------------------------------------------

/// The colors of a navigation item's parts (Compose's `NavigationItemColors`): its icon's and its label's when
/// selected, unselected and disabled, and its indicator's when selected. Equal by its colors.
#[derive(Clone, Copy, PartialEq, Debug, KotlinProperties)]
pub struct NavigationItemColors {
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

impl NavigationItemColors {
    /// Kotlin: `NavigationItemColors(selectedIconColor: Color, selectedTextColor: Color, selectedIndicatorColor: Color, unselectedIconColor: Color, unselectedTextColor: Color, disabledIconColor: Color, disabledTextColor: Color)`
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        selected_icon_color: Color,
        selected_text_color: Color,
        selected_indicator_color: Color,
        unselected_icon_color: Color,
        unselected_text_color: Color,
        disabled_icon_color: Color,
        disabled_text_color: Color,
    ) -> NavigationItemColors {
        NavigationItemColors {
            selected_icon_color,
            selected_text_color,
            selected_indicator_color,
            unselected_icon_color,
            unselected_text_color,
            disabled_icon_color,
            disabled_text_color,
        }
    }

    /// The icon's color, whether the item is `selected` and `enabled` (Compose's `iconColor`): the disabled one
    /// while it isn't enabled.
    pub fn icon_color(&self, selected: bool, enabled: bool) -> Color {
        if !enabled {
            self.disabled_icon_color
        } else if selected {
            self.selected_icon_color
        } else {
            self.unselected_icon_color
        }
    }

    /// The label's color, whether the item is `selected` and `enabled` (Compose's `textColor`): the disabled one
    /// while it isn't enabled.
    pub fn text_color(&self, selected: bool, enabled: bool) -> Color {
        if !enabled {
            self.disabled_text_color
        } else if selected {
            self.selected_text_color
        } else {
            self.unselected_text_color
        }
    }
}

#[named]
impl NavigationItemColors {
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
    ) -> NavigationItemColors {
        NavigationItemColors::new(
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

// ---- The items ---------------------------------------------------------------------------------------------------

/// A navigation suite component's item, such as the [`ShortNavigationBarItem`](crate::ShortNavigationBarItem)
/// (Compose's internal `NavigationItem`): the whole item selectable (the Tab role, `on_click` unless disabled), at
/// least the minimum interactive size each way; its icon (in its color by `colors`) on top of its label
/// (`label_text_style`, in its color) or at its start by `icon_position`, the indicator (`indicator_shape`, in the
/// selected indicator color) behind the icon (or both) growing out of its middle and fading in as it is
/// selected, the ripple over the indicator only (an icon on top: the item's presses moved into the indicator's
/// place, `indicator_width` wide and 12 dp down). The paddings: around the icon (or both) inside the indicator,
/// between the indicator and a label under it, between a start icon and its label, and above and below an item
/// of a top icon.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn NavigationItem(
    selected: bool,
    on_click: Rc<dyn Fn()>,
    icon: Rc<dyn Fn()>,
    label_text_style: TextStyle,
    indicator_shape: Arc<dyn Shape>,
    indicator_width: Dp,
    indicator_horizontal_padding: Dp,
    indicator_vertical_padding: Dp,
    indicator_to_label_vertical_padding: Dp,
    start_icon_to_label_horizontal_padding: Dp,
    top_icon_item_vertical_padding: Dp,
    colors: NavigationItemColors,
    modifier: Modifier,
    enabled: bool,
    label: Option<Rc<dyn Fn()>>,
    icon_position: NavigationItemIconPosition,
    interaction_source: MutableInteractionSource,
) {
    let icon_color = colors.icon_color(selected, enabled);
    let styled_icon: Rc<dyn Fn()> = Rc::new(move || {
        let icon = icon.clone();
        KompozitionLocalProvider(&[LocalContentColor.provides(icon_color)], move || icon());
    });
    let styled_label: Option<Rc<dyn Fn()>> = label.map(|label| {
        let label_text_style = label_text_style.clone();
        Rc::new(move || StyledLabel(selected, label_text_style.clone(), colors, enabled, label.clone())) as Rc<dyn Fn()>
    });

    let item_width = remember(|| mutable_int_state_of(0));
    let set_width = item_width.clone();
    let minimum_size = LocalMinimumInteractiveComponentSize.current();

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
            .default_min_size_with(minimum_size, minimum_size)
            .on_size_changed(move |size| set_width.set(size.width)),
        content_alignment = Alignment::Center,
        propagate_min_constraints = true,
        content = move || {
            let indicator_animation_progress = animate_indicator_progress_as_state(selected);
            let mut offset_interaction_source: Option<MappedInteractionSource> = None;
            if icon_position == NavigationItemIconPosition::Top {
                // The entire item is selectable, but only the indicator pill shows the ripple. To
                // achieve this, we re-map the coordinates of the item's InteractionSource into the
                // coordinates of the indicator.
                offset_interaction_source =
                    Some(remember_offset_interaction_source(&interaction_source, &item_width, indicator_width));
            }

            let progress = indicator_animation_progress.clone();
            NavigationItemLayout(
                offset_interaction_source.map_or_else(|| (&interaction_source).into(), InteractionSource::from),
                colors.selected_indicator_color,
                indicator_shape.clone(),
                styled_icon.clone(),
                icon_position,
                styled_label.clone(),
                Rc::new(move || progress.value().max(0.0)),
                indicator_horizontal_padding,
                indicator_vertical_padding,
                indicator_to_label_vertical_padding,
                start_icon_to_label_horizontal_padding,
                top_icon_item_vertical_padding,
            );
        },
    );
}

/// An animated navigation item, a navigation suite component's such as the wide navigation rail item (Compose's
/// internal `AnimatedNavigationItem`): a [`NavigationItem`] that moves its icon (and grows or shrinks its
/// indicator, its label fading out and in) as `icon_position` changes, on the default spatial spec; the label in
/// `top_icon_label_text_style` while its icon is on top (and less than half way), else in
/// `start_icon_label_text_style`. Its indicator is `indicator_padding` around the icon (or the icon and its label;
/// a [`DynamicPaddingValues`] is told how far the icon has moved), or `no_label_indicator_padding` each side
/// without a label; `item_horizontal_padding` at the item's sides.
#[kompozable]
#[allow(clippy::too_many_arguments)]
pub(crate) fn AnimatedNavigationItem(
    selected: bool,
    on_click: Rc<dyn Fn()>,
    icon: Rc<dyn Fn()>,
    indicator_shape: Arc<dyn Shape>,
    top_icon_indicator_width: Dp,
    top_icon_label_text_style: TextStyle,
    start_icon_label_text_style: TextStyle,
    indicator_padding: PaddingValues,
    top_icon_indicator_to_label_vertical_padding: Dp,
    no_label_indicator_padding: Dp,
    start_icon_to_label_horizontal_padding: Dp,
    item_horizontal_padding: Dp,
    colors: NavigationItemColors,
    modifier: Modifier,
    enabled: bool,
    label: Option<Rc<dyn Fn()>>,
    icon_position: NavigationItemIconPosition,
    interaction_source: MutableInteractionSource,
) {
    let icon_color = colors.icon_color(selected, enabled);
    let styled_icon: Rc<dyn Fn()> = Rc::new(move || {
        let icon = icon.clone();
        KompozitionLocalProvider(&[LocalContentColor.provides(icon_color)], move || icon());
    });

    let item_width = remember(|| mutable_int_state_of(0));
    let set_width = item_width.clone();
    let minimum_size = LocalMinimumInteractiveComponentSize.current();

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
            .default_min_size_with(minimum_size, minimum_size)
            .on_size_changed(move |size| set_width.set(size.width)),
        content_alignment = Alignment::Center,
        propagate_min_constraints = true,
        content = move || {
            let is_icon_position_top = icon_position == NavigationItemIconPosition::Top;
            let indicator_animation_progress = animate_indicator_progress_as_state(selected);
            let icon_position_progress = animate_float_as_state(
                if is_icon_position_top { 0.0 } else { 1.0 },
                // TODO Load the motionScheme tokens from the component tokens file
                animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>() as Arc<dyn AnimationSpec<f32>>,
            );

            // (Compose's `remember { derivedStateOf { … } }`: the styles and the position as first composed, the
            // progress read as it goes)
            let text_style = {
                let (top, start) = (top_icon_label_text_style.clone(), start_icon_label_text_style.clone());
                let progress = icon_position_progress.clone();
                remember(move || {
                    derived_state_of(move || {
                        if is_icon_position_top && progress.value() < 0.5 { top.clone() } else { start.clone() }
                    })
                })
            };
            let styled_label: Option<Rc<dyn Fn()>> = label.clone().map(|label| {
                let text_style = text_style.clone();
                Rc::new(move || StyledLabel(selected, text_style.get(), colors, enabled, label.clone())) as Rc<dyn Fn()>
            });

            let mut offset_interaction_source: Option<MappedInteractionSource> = None;
            if is_icon_position_top {
                // The entire item is selectable, but only the indicator pill shows the ripple. To
                // achieve this, we re-map the coordinates of the item's InteractionSource into the
                // coordinates of the indicator.
                offset_interaction_source = Some(remember_offset_interaction_source(
                    &interaction_source,
                    &item_width,
                    top_icon_indicator_width,
                ));
            }

            let (indicator_progress, position_progress) =
                (indicator_animation_progress.clone(), icon_position_progress.clone());
            AnimatedNavigationItemLayout(
                offset_interaction_source.map_or_else(|| (&interaction_source).into(), InteractionSource::from),
                colors.selected_indicator_color,
                indicator_shape.clone(),
                Rc::new(move || indicator_progress.value().max(0.0)),
                styled_icon.clone(),
                icon_position,
                Rc::new(move || position_progress.value().max(0.0)),
                styled_label,
                indicator_padding.clone(),
                top_icon_indicator_to_label_vertical_padding,
                no_label_indicator_padding,
                start_icon_to_label_horizontal_padding,
                item_horizontal_padding,
            );
        },
    );
}

/// The item's interaction source, its presses moved into the indicator's place: `indicator_width` wide in the
/// middle of the item, [`IndicatorVerticalOffset`] down (Compose's `remember(interactionSource,
/// calculateDeltaOffset) { MappedInteractionSource(…) }`, its lambda kept as long as what it captures is).
#[track_caller]
fn remember_offset_interaction_source(
    interaction_source: &MutableInteractionSource,
    item_width: &kompoz_runtime::MutableState<i32>,
    indicator_width: Dp,
) -> MappedInteractionSource {
    let density = LocalDensity.current();
    let item_width = item_width.clone();
    let source = interaction_source.clone();
    remember_keyed((interaction_source.clone(), density, indicator_width), move || {
        let calculate_delta_offset = move || {
            Offset::new(
                (item_width.get() - density.round_to_px(indicator_width)) as f32 / 2.0,
                density.to_px(IndicatorVerticalOffset),
            )
        };
        MappedInteractionSource::new(&(&source).into(), calculate_delta_offset)
    })
}

// ---- The layouts -------------------------------------------------------------------------------------------------

/// An item's indicator ripple, indicator, icon and label laid out (Compose's private `NavigationItemLayout`): an
/// icon on top of its label (or alone) or at its start. It gives a badge the bounds it keeps in.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn NavigationItemLayout(
    interaction_source: InteractionSource,
    indicator_color: Color,
    indicator_shape: Arc<dyn Shape>,
    icon: Rc<dyn Fn()>,
    icon_position: NavigationItemIconPosition,
    label: Option<Rc<dyn Fn()>>,
    indicator_animation_progress: Rc<dyn Fn() -> f32>,
    indicator_horizontal_padding: Dp,
    indicator_vertical_padding: Dp,
    indicator_to_label_vertical_padding: Dp,
    start_icon_to_label_horizontal_padding: Dp,
    top_icon_item_vertical_padding: Dp,
) {
    let has_label = label.is_some();
    let progress = indicator_animation_progress.clone();
    let content = move || {
        // Create the indicator ripple.
        IndicatorRipple(interaction_source.clone(), indicator_shape.clone());
        // Create the indicator. The indicator has a width-expansion animation which interferes
        // with the timing of the ripple, which is why they are separate composables.
        Indicator(indicator_color, indicator_shape.clone(), progress.clone());

        let icon = icon.clone();
        Box(modifier = Modifier.layout_id(IconLayoutIdTag), content = move || icon());

        if let Some(label) = label.clone() {
            Box(modifier = Modifier.layout_id(LabelLayoutIdTag), content = move || label());
        }
    };
    if !has_label || icon_position == NavigationItemIconPosition::Top {
        Layout(
            modifier = badge_bounds(Modifier),
            content = content,
            measure_policy = TopIconOrIconOnlyMeasurePolicy {
                has_label,
                indicator_animation_progress,
                indicator_horizontal_padding,
                indicator_vertical_padding,
                indicator_to_label_vertical_padding,
                top_icon_item_vertical_padding,
            },
        );
    } else {
        Layout(
            modifier = badge_bounds(Modifier),
            content = content,
            measure_policy = StartIconMeasurePolicy {
                indicator_animation_progress,
                indicator_horizontal_padding,
                indicator_vertical_padding,
                start_icon_to_label_horizontal_padding,
            },
        );
    }
}

/// An animated item's indicator ripple, indicator, icon and label laid out (Compose's private
/// `AnimatedNavigationItemLayout`): moving the icon between its places with a label, a round indicator around a
/// lone icon. It gives a badge the bounds it keeps in.
#[kompozable]
#[allow(clippy::too_many_arguments)]
fn AnimatedNavigationItemLayout(
    interaction_source: InteractionSource,
    indicator_color: Color,
    indicator_shape: Arc<dyn Shape>,
    indicator_animation_progress: Rc<dyn Fn() -> f32>,
    icon: Rc<dyn Fn()>,
    icon_position: NavigationItemIconPosition,
    icon_position_progress: Rc<dyn Fn() -> f32>,
    label: Option<Rc<dyn Fn()>>,
    indicator_padding: PaddingValues,
    top_icon_indicator_to_label_vertical_padding: Dp,
    no_label_indicator_padding: Dp,
    start_icon_to_label_horizontal_padding: Dp,
    item_horizontal_padding: Dp,
) {
    let has_label = label.is_some();
    let progress = indicator_animation_progress.clone();
    let content = move || {
        // Create the indicator ripple.
        IndicatorRipple(interaction_source.clone(), indicator_shape.clone());
        // Create the indicator. The indicator has a width-expansion animation which interferes
        // with the timing of the ripple, which is why they are separate composables.
        Indicator(indicator_color, indicator_shape.clone(), progress.clone());

        let icon = icon.clone();
        Box(modifier = Modifier.layout_id(IconLayoutIdTag), content = move || icon());

        if let Some(label) = label.clone() {
            Box(modifier = Modifier.layout_id(LabelLayoutIdTag), content = move || label());
        }
    };
    if has_label {
        Layout(
            modifier = badge_bounds(Modifier),
            content = content,
            measure_policy = AnimatedMeasurePolicy {
                icon_position,
                icon_position_progress,
                indicator_animation_progress,
                indicator_padding,
                top_icon_indicator_to_label_vertical_padding,
                start_icon_to_label_horizontal_padding,
                item_horizontal_padding,
            },
        );
    } else {
        // If no label, default to circular indicator for the item.
        Layout(
            modifier = badge_bounds(Modifier),
            content = content,
            measure_policy = TopIconOrIconOnlyMeasurePolicy {
                has_label: false,
                indicator_animation_progress,
                indicator_horizontal_padding: no_label_indicator_padding,
                indicator_vertical_padding: no_label_indicator_padding,
                indicator_to_label_vertical_padding: Dp(0.0),
                top_icon_item_vertical_padding: Dp(0.0),
            },
        );
    }
}

/// The child of `measurables` of the layout id `id` (Compose's `fastFirst { it.layoutId == id }`), if any.
fn find_by_id<'a, M: IntrinsicMeasurable + ?Sized>(measurables: &[&'a M], id: &'static str) -> Option<&'a M> {
    measurables.iter().copied().find(|m| m.layout_id().is_some_and(|i| i.is(&id)))
}

/// The child of `measurables` of the layout id `id`, which the layout always has.
fn first_by_id<'a, M: IntrinsicMeasurable + ?Sized>(measurables: &[&'a M], id: &'static str) -> &'a M {
    find_by_id(measurables, id).unwrap_or_else(|| panic!("a navigation item's {id}"))
}

/// `constraints` less their least sizes (Compose's `constraints.copy(minWidth = 0, minHeight = 0)`).
fn loose(constraints: Constraints) -> Constraints {
    Constraints::new(0, constraints.max_width, 0, constraints.max_height)
}

/// An icon on top of its label, or a lone icon (Compose's private `TopIconOrIconOnlyMeasurePolicy`).
struct TopIconOrIconOnlyMeasurePolicy {
    has_label: bool,
    indicator_animation_progress: Rc<dyn Fn() -> f32>,
    indicator_horizontal_padding: Dp,
    indicator_vertical_padding: Dp,
    indicator_to_label_vertical_padding: Dp,
    top_icon_item_vertical_padding: Dp,
}

impl MeasurePolicy for TopIconOrIconOnlyMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let indicator_animation_progress = (self.indicator_animation_progress)();
        let loose_constraints = loose(constraints);
        // When measuring icon, account for the indicator in its constraints.
        let icon_placeable = first_by_id(measurables, IconLayoutIdTag).measure(loose_constraints.offset(
            -scope.round_to_px(self.indicator_horizontal_padding * 2.0),
            -scope.round_to_px(self.indicator_vertical_padding * 2.0),
        ));
        // Next, when measuring the indicator and ripple, still need to obey looseConstraints.
        let total_indicator_width = icon_placeable.width() + scope.round_to_px(self.indicator_horizontal_padding * 2.0);
        let indicator_height = icon_placeable.height() + scope.round_to_px(self.indicator_vertical_padding * 2.0);
        let animated_indicator_width = round_to_int(total_indicator_width as f32 * indicator_animation_progress);
        let indicator_ripple_placeable = first_by_id(measurables, IndicatorRippleLayoutIdTag)
            .measure(loose_constraints.constrain(Constraints::fixed(total_indicator_width, indicator_height)));
        let indicator_placeable = first_by_id(measurables, IndicatorLayoutIdTag)
            .measure(loose_constraints.constrain(Constraints::fixed(animated_indicator_width, indicator_height)));

        if self.has_label {
            // When measuring label, account for the indicator and the padding between indicator and
            // label.
            let label_placeable = first_by_id(measurables, LabelLayoutIdTag).measure(loose_constraints.offset(
                0,
                -(indicator_placeable.height() + scope.round_to_px(self.indicator_to_label_vertical_padding)),
            ));

            place_label_and_top_icon(
                scope,
                label_placeable,
                icon_placeable,
                indicator_ripple_placeable,
                indicator_placeable,
                constraints,
                self.indicator_to_label_vertical_padding,
                self.indicator_vertical_padding,
                self.top_icon_item_vertical_padding,
            )
        } else {
            place_icon(scope, icon_placeable, indicator_ripple_placeable, indicator_placeable, constraints)
        }
    }

    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        let icon_height = first_by_id(measurables, IconLayoutIdTag).max_intrinsic_height(width);
        let label_height = find_by_id(measurables, LabelLayoutIdTag).map_or(0, |m| m.max_intrinsic_height(width));
        let paddings = scope.round_to_px(
            self.top_icon_item_vertical_padding * 2.0
                + self.indicator_vertical_padding * 2.0
                + self.indicator_to_label_vertical_padding,
        );

        icon_height + label_height + paddings
    }
}

/// An icon at the start of its label (Compose's private `StartIconMeasurePolicy`).
struct StartIconMeasurePolicy {
    indicator_animation_progress: Rc<dyn Fn() -> f32>,
    indicator_horizontal_padding: Dp,
    indicator_vertical_padding: Dp,
    start_icon_to_label_horizontal_padding: Dp,
}

impl MeasurePolicy for StartIconMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let indicator_animation_progress = (self.indicator_animation_progress)();
        let loose_constraints = loose(constraints);
        // When measuring icon, account for the indicator in its constraints.
        let icon_placeable = first_by_id(measurables, IconLayoutIdTag).measure(loose_constraints);
        // When measuring the label, account for the indicator, the icon, and the padding between
        // icon and label.
        let label_placeable = first_by_id(measurables, LabelLayoutIdTag).measure(
            loose_constraints
                .offset(-(icon_placeable.width() + scope.round_to_px(self.start_icon_to_label_horizontal_padding)), 0),
        );

        let total_indicator_width = icon_placeable.width()
            + label_placeable.width()
            + scope.round_to_px(self.start_icon_to_label_horizontal_padding + self.indicator_horizontal_padding * 2.0);
        let indicator_height = icon_placeable.height().max(label_placeable.height())
            + scope.round_to_px(self.indicator_vertical_padding * 2.0);
        let animated_indicator_width = round_to_int(total_indicator_width as f32 * indicator_animation_progress);
        // When measuring the indicator and ripple, still need to obey looseConstraints.
        let indicator_ripple_placeable = first_by_id(measurables, IndicatorRippleLayoutIdTag)
            .measure(loose_constraints.constrain(Constraints::fixed(total_indicator_width, indicator_height)));
        let indicator_placeable = first_by_id(measurables, IndicatorLayoutIdTag)
            .measure(loose_constraints.constrain(Constraints::fixed(animated_indicator_width, indicator_height)));

        place_label_and_start_icon(
            scope,
            label_placeable,
            icon_placeable,
            indicator_ripple_placeable,
            indicator_placeable,
            constraints,
            self.start_icon_to_label_horizontal_padding,
        )
    }

    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        let icon_width = first_by_id(measurables, IconLayoutIdTag).max_intrinsic_width(height);
        let label_width = first_by_id(measurables, LabelLayoutIdTag).max_intrinsic_width(height);
        let paddings =
            scope.round_to_px(self.indicator_horizontal_padding * 2.0 + self.start_icon_to_label_horizontal_padding);

        icon_width + label_width + paddings
    }

    fn max_intrinsic_height(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        let icon_height = first_by_id(measurables, IconLayoutIdTag).max_intrinsic_height(width);
        let label_height = first_by_id(measurables, LabelLayoutIdTag).max_intrinsic_height(width);
        let paddings = scope.round_to_px(self.indicator_vertical_padding * 2.0);

        icon_height.max(label_height) + paddings
    }
}

/// An icon moving between the top of its label and its start (Compose's private `AnimatedMeasurePolicy`).
struct AnimatedMeasurePolicy {
    icon_position: NavigationItemIconPosition,
    icon_position_progress: Rc<dyn Fn() -> f32>,
    indicator_animation_progress: Rc<dyn Fn() -> f32>,
    indicator_padding: PaddingValues,
    top_icon_indicator_to_label_vertical_padding: Dp,
    start_icon_to_label_horizontal_padding: Dp,
    item_horizontal_padding: Dp,
}

impl MeasurePolicy for AnimatedMeasurePolicy {
    fn measure(
        &self,
        scope: &MeasureScope,
        measurables: &[&dyn Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let indicator_animation_progress = (self.indicator_animation_progress)();
        let icon_position_progress_value = (self.icon_position_progress)();
        let loose_constraints = loose(constraints);
        let layout_direction = scope.layout_direction();

        let icon_placeable = first_by_id(measurables, IconLayoutIdTag).measure(loose_constraints);

        let label_placeable = first_by_id(measurables, LabelLayoutIdTag).measure(loose_constraints);

        if let Some(dynamic) = self.indicator_padding.custom_as::<DynamicPaddingValues>() {
            dynamic.set_progress(icon_position_progress_value);
        }

        let indicator_horizontal_padding = self.indicator_padding.calculate_start_padding(layout_direction)
            + self.indicator_padding.calculate_end_padding(layout_direction);
        let indicator_vertical_padding =
            self.indicator_padding.calculate_top_padding() + self.indicator_padding.calculate_bottom_padding();
        let top_icon_indicator_width = icon_placeable.width();
        let top_icon_indicator_height = icon_placeable.height();

        let start_icon_indicator_width = icon_placeable.width()
            + label_placeable.width()
            + scope.round_to_px(self.start_icon_to_label_horizontal_padding);

        let start_icon_indicator_height = icon_placeable.height().max(label_placeable.height());

        let indicator_width_progress =
            lerp_int(top_icon_indicator_width, start_icon_indicator_width, icon_position_progress_value)
                + scope.round_to_px(indicator_horizontal_padding);
        let animated_indicator_width = round_to_int(indicator_width_progress as f32 * indicator_animation_progress);
        let indicator_height_progress =
            lerp_int(top_icon_indicator_height, start_icon_indicator_height, icon_position_progress_value)
                + scope.round_to_px(indicator_vertical_padding);

        let indicator_ripple_placeable = first_by_id(measurables, IndicatorRippleLayoutIdTag).measure(
            loose_constraints.constrain(Constraints::fixed(indicator_width_progress, indicator_height_progress)),
        );
        let indicator_placeable = first_by_id(measurables, IndicatorLayoutIdTag).measure(
            loose_constraints.constrain(Constraints::fixed(animated_indicator_width, indicator_height_progress)),
        );

        place_animated_label_and_icon(
            scope,
            self.icon_position,
            &*self.icon_position_progress,
            label_placeable,
            icon_placeable,
            indicator_ripple_placeable,
            indicator_placeable,
            top_icon_indicator_width,
            loose_constraints,
            self.top_icon_indicator_to_label_vertical_padding,
            &self.indicator_padding,
            self.start_icon_to_label_horizontal_padding,
            self.item_horizontal_padding,
        )
    }

    fn max_intrinsic_width(
        &self,
        scope: &IntrinsicMeasureScope,
        measurables: &[&dyn IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        let icon_width = first_by_id(measurables, IconLayoutIdTag).max_intrinsic_width(height);
        let label_width = first_by_id(measurables, LabelLayoutIdTag).max_intrinsic_width(height);
        let layout_direction = scope.layout_direction();

        let indicator_horizontal_padding = scope.round_to_px(
            self.indicator_padding.calculate_start_padding(layout_direction)
                + self.indicator_padding.calculate_end_padding(layout_direction),
        );

        if self.icon_position == NavigationItemIconPosition::Top {
            let paddings = indicator_horizontal_padding + scope.round_to_px(self.item_horizontal_padding * 2.0);
            label_width.max(icon_width + paddings)
        } else {
            let paddings = indicator_horizontal_padding
                + scope.round_to_px(self.start_icon_to_label_horizontal_padding + self.item_horizontal_padding);
            icon_width + label_width + paddings
        }
    }
}

/// A lone icon in the middle of its indicator (Compose's private `placeIcon`).
fn place_icon(
    scope: &MeasureScope,
    icon_placeable: Placeable,
    indicator_ripple_placeable: Placeable,
    indicator_placeable: Placeable,
    constraints: Constraints,
) -> MeasureResult {
    let width = constraints.constrain_width(indicator_ripple_placeable.width());
    let height = constraints.constrain_height(indicator_ripple_placeable.height());

    let indicator_x = (width - indicator_placeable.width()) / 2;
    let indicator_y = (height - indicator_placeable.height()) / 2;
    let icon_x = (width - icon_placeable.width()) / 2;
    let icon_y = (height - icon_placeable.height()) / 2;
    let ripple_x = (width - indicator_ripple_placeable.width()) / 2;
    let ripple_y = (height - indicator_ripple_placeable.height()) / 2;

    scope.layout(width, height, move |p| {
        p.place_relative(&indicator_placeable, indicator_x, indicator_y);
        p.place_relative(&icon_placeable, icon_x, icon_y);
        p.place_relative(&indicator_ripple_placeable, ripple_x, ripple_y);
    })
}

/// An icon (in its indicator) a set distance above its label, both centered (Compose's private
/// `placeLabelAndTopIcon`).
#[allow(clippy::too_many_arguments)]
fn place_label_and_top_icon(
    scope: &MeasureScope,
    label_placeable: Placeable,
    icon_placeable: Placeable,
    indicator_ripple_placeable: Placeable,
    indicator_placeable: Placeable,
    constraints: Constraints,
    indicator_to_label_vertical_padding: Dp,
    indicator_vertical_padding: Dp,
    top_icon_item_vertical_padding: Dp,
) -> MeasureResult {
    let width = constraints.constrain_width(label_placeable.width().max(indicator_ripple_placeable.width()));
    let content_height = indicator_ripple_placeable.height() as f32
        + scope.to_px(indicator_to_label_vertical_padding)
        + label_placeable.height() as f32;
    let height =
        constraints.constrain_height(round_to_int(content_height + scope.to_px(top_icon_item_vertical_padding) * 2.0));

    let icon_y = scope.round_to_px(top_icon_item_vertical_padding + indicator_vertical_padding);
    let icon_x = (width - icon_placeable.width()) / 2;
    let indicator_x = (width - indicator_placeable.width()) / 2;
    let indicator_y = icon_y - scope.round_to_px(indicator_vertical_padding);
    let label_x = (width - label_placeable.width()) / 2;
    // Label should be fixed padding below icon.
    let label_y = icon_y
        + icon_placeable.height()
        + scope.round_to_px(indicator_vertical_padding + indicator_to_label_vertical_padding);
    let ripple_x = (width - indicator_ripple_placeable.width()) / 2;
    let ripple_y = indicator_y;

    scope.layout(width, height, move |p| {
        p.place_relative(&indicator_placeable, indicator_x, indicator_y);
        p.place_relative(&label_placeable, label_x, label_y);
        p.place_relative(&icon_placeable, icon_x, icon_y);
        p.place_relative(&indicator_ripple_placeable, ripple_x, ripple_y);
    })
}

/// An icon at the start of its label, together in the middle of their indicator (Compose's private
/// `placeLabelAndStartIcon`).
fn place_label_and_start_icon(
    scope: &MeasureScope,
    label_placeable: Placeable,
    icon_placeable: Placeable,
    indicator_ripple_placeable: Placeable,
    indicator_placeable: Placeable,
    constraints: Constraints,
    start_icon_to_label_horizontal_padding: Dp,
) -> MeasureResult {
    let width = constraints.constrain_width(indicator_ripple_placeable.width());
    let height = constraints.constrain_height(indicator_ripple_placeable.height());

    let indicator_x = (width - indicator_placeable.width()) / 2;
    let indicator_y = (height - indicator_placeable.height()) / 2;
    let icon_y = (height - icon_placeable.height()) / 2;
    let label_y = (height - label_placeable.height()) / 2;
    let item_content_width =
        icon_placeable.width() + scope.round_to_px(start_icon_to_label_horizontal_padding) + label_placeable.width();
    let icon_x = (width - item_content_width) / 2;
    let label_x = icon_x + icon_placeable.width() + scope.round_to_px(start_icon_to_label_horizontal_padding);
    let ripple_x = (width - indicator_ripple_placeable.width()) / 2;
    let ripple_y = (height - indicator_ripple_placeable.height()) / 2;

    scope.layout(width, height, move |p| {
        p.place_relative(&indicator_placeable, indicator_x, indicator_y);
        p.place_relative(&label_placeable, label_x, label_y);
        p.place_relative(&icon_placeable, icon_x, icon_y);
        p.place_relative(&indicator_ripple_placeable, ripple_x, ripple_y);
    })
}

/// An icon and its label placed `icon_position_progress()` of the way from the icon on top to the icon at the
/// start, the label fading out and in at the half way (Compose's private `placeAnimatedLabelAndIcon`).
#[allow(clippy::too_many_arguments)]
fn place_animated_label_and_icon(
    scope: &MeasureScope,
    icon_position: NavigationItemIconPosition,
    icon_position_progress: &dyn Fn() -> f32,
    label_placeable: Placeable,
    icon_placeable: Placeable,
    indicator_ripple_placeable: Placeable,
    indicator_placeable: Placeable,
    top_icon_indicator_width: i32,
    constraints: Constraints,
    top_icon_indicator_to_label_vertical_padding: Dp,
    indicator_padding: &PaddingValues,
    start_icon_to_label_horizontal_padding: Dp,
    item_horizontal_padding: Dp,
) -> MeasureResult {
    let icon_position_progress = icon_position_progress();
    let layout_direction = scope.layout_direction();
    let is_icon_position_top = icon_position == NavigationItemIconPosition::Top;
    let width_top_icon = constraints.constrain_width(label_placeable.width().max(
        (top_icon_indicator_width + scope.round_to_px(item_horizontal_padding * 2.0))
            + scope.round_to_px(
                indicator_padding.calculate_end_padding(layout_direction)
                    + indicator_padding.calculate_start_padding(layout_direction),
            ),
    ));
    let width_start_icon =
        constraints.constrain_width(indicator_ripple_placeable.width() + scope.round_to_px(item_horizontal_padding));
    let width = width_top_icon as f32 + (width_start_icon - width_top_icon) as f32 * icon_position_progress;
    let height_top_icon = constraints.constrain_height(round_to_int(
        indicator_ripple_placeable.height() as f32
            + scope.to_px(top_icon_indicator_to_label_vertical_padding)
            + label_placeable.height() as f32,
    ));
    let height_start_icon = constraints.constrain_height(indicator_ripple_placeable.height());
    let height = lerp_int(height_top_icon, height_start_icon, icon_position_progress);

    let ripple_x_top_icon = scope.round_to_px(item_horizontal_padding);
    let ripple_x_start_icon = round_to_int(
        (scope.round_to_px(item_horizontal_padding) as f32 + width - indicator_ripple_placeable.width() as f32) / 2.0,
    );
    let ripple_x = lerp_int(ripple_x_top_icon, ripple_x_start_icon, icon_position_progress);

    let indicator_x = scope.round_to_px(item_horizontal_padding);
    let icon_x = scope.round_to_px(item_horizontal_padding)
        + scope.round_to_px(indicator_padding.calculate_start_padding(layout_direction));

    let icon_y_top_icon = scope.round_to_px(indicator_padding.calculate_top_padding());
    let icon_y_start_icon = (height - icon_placeable.height()) / 2 - icon_y_top_icon;

    let icon_y = lerp_int(0, icon_y_start_icon, icon_position_progress) + icon_y_top_icon;
    let indicator_horizontal_padding = indicator_padding.calculate_start_padding(layout_direction)
        + indicator_padding.calculate_end_padding(layout_direction);

    let label_x_top_icon = ((icon_placeable.width()
        + scope.round_to_px(indicator_horizontal_padding + item_horizontal_padding * 2.0))
        - label_placeable.width())
        / 2;

    let label_y_top_icon = icon_y
        + icon_placeable.height()
        + scope
            .round_to_px(indicator_padding.calculate_bottom_padding() + top_icon_indicator_to_label_vertical_padding);

    let label_x_start_icon_horizontal_offset = if is_icon_position_top && icon_position_progress > 0.0 {
        0.0
    } else {
        scope.round_to_px(item_horizontal_padding) as f32 * (1.0 - icon_position_progress)
    };
    let label_x_start_icon =
        (icon_x + icon_placeable.width() + scope.round_to_px(start_icon_to_label_horizontal_padding)) as f32
            - label_x_start_icon_horizontal_offset;
    let label_y_start_icon = (height - label_placeable.height()) / 2;
    // (Kotlin's `if` of an Int and a Float, then `toInt()`: the Float's truncated)
    let label_x = if icon_position_progress < 0.5 {
        label_x_top_icon
    } else {
        (label_x_start_icon * icon_position_progress) as i32
    };
    let label_y = if icon_position_progress < 0.5 { label_y_top_icon } else { label_y_start_icon };
    scope.layout(round_to_int(width), height, move |p| {
        p.place_relative_with_layer(&indicator_placeable, indicator_x, 0, |_| {});
        p.place_relative_with_layer(&icon_placeable, icon_x, icon_y, |_| {});
        p.place_relative_with_layer(&label_placeable, label_x, label_y, move |layer| {
            layer.alpha = 4.0 * (icon_position_progress - 0.5) * (icon_position_progress - 0.5);
        });
        p.place_relative_with_layer(&indicator_ripple_placeable, ripple_x, 0, |_| {});
    })
}

/// A label in its color and `label_text_style` (Compose's private `StyledLabel`).
#[kompozable]
fn StyledLabel(
    selected: bool,
    label_text_style: TextStyle,
    colors: NavigationItemColors,
    enabled: bool,
    content: Rc<dyn Fn()>,
) {
    let text_color = colors.text_color(selected, enabled);
    ProvideContentColorTextStyle(text_color, label_text_style, move || content());
}

/// The indicator's progress, to 1 as the item is selected and back to 0, on the default spatial spec (Compose's
/// private `animateIndicatorProgressAsState`).
#[kompozable]
fn animate_indicator_progress_as_state(selected: bool) -> State<f32> {
    animate_float_as_state(
        if selected { 1.0 } else { 0.0 },
        // TODO Load the motionScheme tokens from the component tokens file
        animation_spec = MotionSchemeKeyTokens::DefaultSpatial.value::<f32>() as Arc<dyn AnimationSpec<f32>>,
    )
}

/// The ripple over the indicator, clipped to its shape (Compose's private `IndicatorRipple`).
#[kompozable]
fn IndicatorRipple(interaction_source: InteractionSource, indicator_shape: Arc<dyn Shape>) {
    Box(modifier = Modifier
        .layout_id(IndicatorRippleLayoutIdTag)
        .clip(ClipShape(indicator_shape))
        .indication(interaction_source, Some(ripple(true, Dp::Unspecified, Color::Unspecified))));
}

/// The indicator: its shape in its color, faded by its progress (Compose's private `Indicator`).
#[kompozable]
fn Indicator(
    indicator_color: Color,
    indicator_shape: Arc<dyn Shape>,
    indicator_animation_progress: Rc<dyn Fn() -> f32>,
) {
    Box(modifier = Modifier
        .layout_id(IndicatorLayoutIdTag)
        .graphics_layer_block(move |layer| {
            layer.compositing_strategy = CompositingStrategy::ModulateAlpha;
            layer.alpha = indicator_animation_progress();
        })
        .background(indicator_color, indicator_shape));
}

// ---- DynamicPaddingValues ----------------------------------------------------------------------------------------

/// Padding between a collapsed and an expanded one, `progress` of the way (Compose's internal
/// `DynamicPaddingValues`): a wide navigation rail item's indicator padding, its progress set by the item's
/// layout as its icon moves (at 0 the collapsed padding, at 1 the expanded). Given as padding values by
/// [`PaddingValues::custom`] (the item's layout finds it by [`PaddingValues::custom_as`]).
pub(crate) struct DynamicPaddingValues {
    pub(crate) collapsed_padding_values: PaddingValues,
    pub(crate) expanded_padding_values: PaddingValues,
    progress: Cell<f32>,
}

impl DynamicPaddingValues {
    /// Padding at `collapsed_padding_values` or, `is_expanded`, at `expanded_padding_values`.
    pub(crate) fn new(
        collapsed_padding_values: PaddingValues,
        expanded_padding_values: PaddingValues,
        is_expanded: bool,
    ) -> DynamicPaddingValues {
        DynamicPaddingValues {
            collapsed_padding_values,
            expanded_padding_values,
            progress: Cell::new(if is_expanded { 1.0 } else { 0.0 }),
        }
    }

    /// How far from the collapsed padding to the expanded one it is (Compose's `progress`).
    pub(crate) fn progress(&self) -> f32 {
        self.progress.get()
    }

    /// Sets how far it is, kept within 0 and 1 (Compose's `progress` setter).
    pub(crate) fn set_progress(&self, value: f32) {
        self.progress.set(value.clamp(0.0, 1.0));
    }
}

impl CustomPaddingValues for DynamicPaddingValues {
    fn calculate_left_padding(&self, layout_direction: LayoutDirection) -> Dp {
        lerp_dp(
            self.collapsed_padding_values.calculate_left_padding(layout_direction),
            self.expanded_padding_values.calculate_left_padding(layout_direction),
            self.progress(),
        )
    }

    fn calculate_top_padding(&self) -> Dp {
        lerp_dp(
            self.collapsed_padding_values.calculate_top_padding(),
            self.expanded_padding_values.calculate_top_padding(),
            self.progress(),
        )
    }

    fn calculate_right_padding(&self, layout_direction: LayoutDirection) -> Dp {
        lerp_dp(
            self.collapsed_padding_values.calculate_right_padding(layout_direction),
            self.expanded_padding_values.calculate_right_padding(layout_direction),
            self.progress(),
        )
    }

    fn calculate_bottom_padding(&self) -> Dp {
        lerp_dp(
            self.collapsed_padding_values.calculate_bottom_padding(),
            self.expanded_padding_values.calculate_bottom_padding(),
            self.progress(),
        )
    }
}

// ---- Helpers and constants ---------------------------------------------------------------------------------------

/// Kotlin's `Float.roundToInt` (Math.round: a half up).
fn round_to_int(value: f32) -> i32 {
    (value + 0.5).floor() as i32
}

/// Compose's `lerp(start: Int, stop: Int, fraction: Float)` (ui.util): the difference scaled in doubles, rounded
/// a half up.
fn lerp_int(start: i32, stop: i32, fraction: f32) -> i32 {
    start + ((stop - start) as f64 * fraction as f64 + 0.5).floor() as i32
}

const IndicatorRippleLayoutIdTag: &str = "indicatorRipple";
const IndicatorLayoutIdTag: &str = "indicator";
const IconLayoutIdTag: &str = "icon";
const LabelLayoutIdTag: &str = "label";

const IndicatorVerticalOffset: Dp = Dp(12.0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_padding_values_lerp_by_their_progress() {
        let dynamic =
            DynamicPaddingValues::new(PaddingValues::all(Dp(4.0)), PaddingValues::hv(Dp(16.0), Dp(8.0)), false);
        assert_eq!(dynamic.calculate_left_padding(LayoutDirection::Ltr), Dp(4.0));
        dynamic.set_progress(0.5);
        assert_eq!(dynamic.calculate_left_padding(LayoutDirection::Rtl), Dp(10.0));
        assert_eq!(dynamic.calculate_top_padding(), Dp(6.0));
        dynamic.set_progress(2.0); // (kept within 0 and 1)
        assert_eq!((dynamic.progress(), dynamic.calculate_bottom_padding()), (1.0, Dp(8.0)));
        assert_eq!(DynamicPaddingValues::new(PaddingValues::Zero, PaddingValues::Zero, true).progress(), 1.0);
    }

    #[test]
    fn int_lerp_and_rounding_are_kotlins() {
        assert_eq!((lerp_int(0, 10, 0.25), lerp_int(10, 0, 0.25), lerp_int(0, 3, 0.5)), (3, 8, 2));
        assert_eq!((round_to_int(2.5), round_to_int(-2.5), round_to_int(-2.6)), (3, -2, -3));
        assert_eq!(format!("{} {:?}", NavigationItemIconPosition::Top, NavigationItemIconPosition::Start), "Top Start");
    }
}
